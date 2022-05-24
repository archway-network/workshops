# Archway Developer Rewards

In this workshop, we show how to use the Archway rewards system in different ways.

- [Presentation slides](./presentation/Archway-Developer-Rewards-with-CosmWasm.pdf)

## Setup

### Create the wallets

```bash
archway accounts -a deployer
archway accounts -a alice
archway accounts -a arbiter
archway accounts -a recipient
```

### Request for funds from the faucet

The faucet for Titus and Constantine networks is in our Discord server. You can find the instructions to use it running the following command:

```bash
archway faucet
```

The accounts that should be funded for this demo are the `deployer` and `arbiter`.

### Deploy the increment contract

```bash
cd contracts/increment
archway deploy --no-confirm --from deployer --default-label --args '{ "count": 0 }'
```

## Using the rewards

### Gas Rebates

#### Benchmark

Start with a basic benchmark of gas consumed by a normal transaction:

##### Interact with the contract

```bash
archway tx --no-confirm --from deployer --args '{ "increment": {} }'
```

This will output a transaction hash (txhash) that we should use in the next command.

##### Check the gas consumed

```bash
TXHASH="...hash from previous command..."
archwayd q tx $TXHASH \
    --node 'https://rpc.constantine-1.archway.tech:443' \
    --output json \
    | jq '.tx.auth_info.fee.amount[]'
```

#### Enable gas rebates

Now, update the contract metadata to enable gas rebates:

```bash
archway metadata \
    --no-confirm \
    --from deployer \
    --developer-address $(archwayd keys show -a deployer) \
    --reward-address $(archwayd keys show -a alice) \
    --gas-rebate
```

#### Interact with the contract again

```bash
archway tx --no-confirm --from deployer --args '{ "increment": {} }'
```

#### Check the gas consumed

```bash
TXHASH="...hash from previous command..."
archwayd q tx $TXHASH \
    --node 'https://rpc.constantine-1.archway.tech:443' \
    --output json \
    | jq '.tx.auth_info.fee.amount[]'
```

The gas consumed should be less than in the previous transaction (before setting the metadata)! 🤑

#### Check the reward address balance

```bash
archwayd q bank balances $(archwayd keys show -a alice) \
    --node 'https://rpc.constantine-1.archway.tech:443' \
    --output json \
    | jq '.balances[]'
```

The balance should be **empty**.

### Collect Premium

#### Enable a 200% premium on transaction fees

```bash
archway metadata \
    --no-confirm \
    --from deployer \
    --developer-address $(archwayd keys show -a deployer) \
    --reward-address $(archwayd keys show -a alice) \
    --collect-premium \
    --premium-percentage 200
```

#### Interact with the contract

```bash
archway tx --no-confirm --from deployer --args '{ "increment": {} }'
```

#### Check the reward address balance

```bash
archwayd q bank balances $(archwayd keys show -a alice) \
    --node 'https://rpc.constantine-1.archway.tech:443' \
    --output json \
    | jq '.balances[]'
```

Alice should have received the rewards! 🙌🏻

### Using another contract as the rewards destination

#### Deploy the rewards escrow contract

The escrow contract has 3 "personas":

- A sender, which is the account that instantiated the contract (in this case, the `deployer` account).
- A `recipient`, to receive the funds in the escrow.
- An `arbiter`, that will authorize the funds to the `recipient` or refund the tokens to the sender.

```bash
cd contracts/rewards-escrow

INIT_MSG="$(jq --null-input \
    --arg arbiter "$(archwayd keys show -a arbiter)" \
    --arg recipient "$(archwayd keys show -a recipient)" \
    '{ arbiter: $arbiter, recipient: $recipient }')"

archway deploy --no-confirm --from deployer --default-label --args "$INIT_MSG"

ESCROW_ADDRESS=$(jq -r '.developer.deployments[] | first(select(.type == "instantiate")) | .address' config.json)
```

We will use the contract address saved in `$ESCROW_ADDRESS` on the next step.


#### Set the increment contract metadata

Go back to the `contracts/increment` folder and set the escrow contract address as the rewards destination:

```bash
archway metadata \
    --no-confirm \
    --from deployer \
    --developer-address $(archwayd keys show -a deployer) \
    --reward-address $ESCROW_ADDRESS \
    --collect-premium \
    --premium-percentage 200
```

#### Interact with the increment contract

```bash
archway tx --no-confirm --from deployer --args '{ "increment": {} }'
```

#### Check the escrow contract balance

```bash
archwayd q bank balances $ESCROW_ADDRESS \
    --node 'https://rpc.constantine-1.archway.tech:443' \
    --output json \
    | jq '.balances[]'
```

#### Authorize sending the funds to the recipient

Go back to the `contracts/rewards-escrow` directory and interact with the contract:

```bash
archway tx --no-confirm --from arbiter --args '{ "approve": {} }'
```

#### Check the recipient and escrow contract balances

```bash
archwayd q bank balances $ESCROW_ADDRESS \
    --node 'https://rpc.constantine-1.archway.tech:443' \
    --output json \
    | jq '.balances[]'

archwayd q bank balances $(archwayd keys show -a recipient) \
    --node 'https://rpc.constantine-1.archway.tech:443' \
    --output json \
    | jq '.balances[]'
```

Now, the funds should have been transferred from the contract to the `recipient`! 🎉
