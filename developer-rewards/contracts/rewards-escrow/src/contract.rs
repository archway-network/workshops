use cosmwasm_std::{
    entry_point, to_binary, Addr, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult,
};

use crate::error::ContractError;
use crate::msg::{ArbiterResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{config, config_read, State};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        arbiter: deps.api.addr_validate(&msg.arbiter)?,
        recipient: deps.api.addr_validate(&msg.recipient)?,
        source: info.sender,
    };

    config(deps.storage).save(&state)?;
    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let state = config_read(deps.storage).load()?;
    match msg {
        ExecuteMsg::Approve { quantity } => try_approve(deps, env, state, info, quantity),
        ExecuteMsg::Refund {} => try_refund(deps, env, info, state),
    }
}

fn try_approve(
    deps: DepsMut,
    env: Env,
    state: State,
    info: MessageInfo,
    quantity: Option<Vec<Coin>>,
) -> Result<Response, ContractError> {
    if info.sender != state.arbiter {
        return Err(ContractError::Unauthorized {});
    }

    let amount = if let Some(quantity) = quantity {
        quantity
    } else {
        // Querier guarantees to returns up-to-date data, including funds sent in this handle message
        // https://github.com/CosmWasm/wasmd/blob/master/x/wasm/internal/keeper/keeper.go#L185-L192
        deps.querier.query_all_balances(&env.contract.address)?
    };

    Ok(send_tokens(state.recipient, amount, "approve"))
}

fn try_refund(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    state: State,
) -> Result<Response, ContractError> {
    if info.sender != state.arbiter {
        return Err(ContractError::Unauthorized {});
    }

    // Querier guarantees to returns up-to-date data, including funds sent in this handle message
    // https://github.com/CosmWasm/wasmd/blob/master/x/wasm/internal/keeper/keeper.go#L185-L192
    let balance = deps.querier.query_all_balances(&env.contract.address)?;
    Ok(send_tokens(state.source, balance, "refund"))
}

// this is a helper to move the tokens, so the business logic is easy to read
fn send_tokens(to_address: Addr, amount: Vec<Coin>, action: &str) -> Response {
    Response::new()
        .add_message(BankMsg::Send {
            to_address: to_address.clone().into(),
            amount,
        })
        .add_attribute("action", action)
        .add_attribute("to", to_address)
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Arbiter {} => to_binary(&query_arbiter(deps)?),
    }
}

fn query_arbiter(deps: Deps) -> StdResult<ArbiterResponse> {
    let state = config_read(deps.storage).load()?;
    let addr = state.arbiter;
    Ok(ArbiterResponse { arbiter: addr })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, CosmosMsg};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            arbiter: String::from("verifies"),
            recipient: String::from("benefits"),
        };
        let env = mock_env();
        let info = mock_info("creator", &coins(1000, "earth"));

        let res = instantiate(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let state = config_read(&mut deps.storage).load().unwrap();
        assert_eq!(
            state,
            State {
                arbiter: Addr::unchecked("verifies"),
                recipient: Addr::unchecked("benefits"),
                source: Addr::unchecked("creator"),
            }
        );
    }

    #[test]
    fn init_and_query() {
        let mut deps = mock_dependencies();

        let arbiter = Addr::unchecked("arbiters");
        let recipient = Addr::unchecked("receives");
        let creator = Addr::unchecked("creates");
        let msg = InstantiateMsg {
            arbiter: arbiter.clone().into(),
            recipient: recipient.into(),
        };
        let env = mock_env();
        let info = mock_info(creator.as_str(), &[]);
        let res = instantiate(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // now let's query
        let query_response = query_arbiter(deps.as_ref()).unwrap();
        assert_eq!(query_response.arbiter, arbiter);
    }

    #[test]
    fn execute_approve() {
        let mut deps = mock_dependencies();

        // initialize the store
        let init_amount = coins(1000, "earth");
        let msg = InstantiateMsg {
            arbiter: String::from("verifies"),
            recipient: String::from("benefits"),
        };
        let env = mock_env();
        let info = mock_info("creator", &init_amount);
        let contract_addr = env.clone().contract.address;
        let init_res = instantiate(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(0, init_res.messages.len());

        // balance changed in init
        deps.querier.update_balance(&contract_addr, init_amount);

        // beneficiary cannot release it
        let msg = ExecuteMsg::Approve { quantity: None };
        let env = mock_env();
        let info = mock_info("beneficiary", &[]);
        let execute_res = execute(deps.as_mut(), env, info, msg.clone());
        match execute_res.unwrap_err() {
            ContractError::Unauthorized { .. } => {}
            e => panic!("unexpected error: {:?}", e),
        }

        // complete release by verifier, before expiration
        let env = mock_env();
        let info = mock_info("verifies", &[]);
        let execute_res = execute(deps.as_mut(), env, info, msg.clone()).unwrap();
        assert_eq!(1, execute_res.messages.len());
        let msg = execute_res.messages.get(0).expect("no message");
        assert_eq!(
            msg.msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "benefits".into(),
                amount: coins(1000, "earth"),
            })
        );

        // partial release by verifier, before expiration
        let partial_msg = ExecuteMsg::Approve {
            quantity: Some(coins(500, "earth")),
        };
        let env = mock_env();
        let info = mock_info("verifies", &[]);
        let execute_res = execute(deps.as_mut(), env, info, partial_msg).unwrap();
        assert_eq!(1, execute_res.messages.len());
        let msg = execute_res.messages.get(0).expect("no message");
        assert_eq!(
            msg.msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "benefits".into(),
                amount: coins(500, "earth"),
            })
        );
    }

    #[test]
    fn handle_refund() {
        let mut deps = mock_dependencies();

        // initialize the store
        let init_amount = coins(1000, "earth");
        let msg = InstantiateMsg {
            arbiter: String::from("verifies"),
            recipient: String::from("benefits"),
        };
        let env = mock_env();
        let info = mock_info("creator", &init_amount);
        let contract_addr = env.clone().contract.address;
        let init_res = instantiate(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(0, init_res.messages.len());

        // balance changed in init
        deps.querier.update_balance(&contract_addr, init_amount);

        // anyone can release after expiration
        let env = mock_env();
        let msg = ExecuteMsg::Refund {};
        let info = mock_info("anybody", &[]);
        let execute_res = execute(deps.as_mut(), env, info, msg.clone());
        match execute_res.unwrap_err() {
            ContractError::Unauthorized { .. } => {}
            e => panic!("unexpected error: {:?}", e),
        }
    }
}
