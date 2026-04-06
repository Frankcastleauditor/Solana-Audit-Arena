## Project overview

This project uses a bonding curve to create and distribute tokens to purchasers (using SOL). Once the target amount of SOL enters the bonding curve, some SOL and tokens will be reserved as a fee for the platform, and the bonding curve should be disabled. (Outside the scope of this program, the remaining liquidity should be added to Meteora.)

The project should execute according to the following parameters:

Supply of Tokens:
    1,000,000,000

Escape Bonding Curve Amount (Tokens purchased by buyers):
    800,000,000

Escape Bonding Curve Token Reserve (Set aside for liquidity and platform fees):
    200,000,000

Graviton Tokens Fee (Amount of tokens platform will keep as a fee):
    6,900,000

Target USD Reserve (Collected in SOL but mapped to USD):
    $10,000.00

Escape Fee (% of Target USD Reserve):
    10.69%

Escape Fee (Collected in SOL but mapped to USD):
    $1,069.00

Trading Fee (% of SOL traded in the bonding curve):
    1%

**Parameters aren’t hardcoded, they are config settings the authority can update. When the program is deployed, an account is created with a configuration including the wallet address for the authority.**

## Audit competition scope

Everything inside the "programs/tokens" folder is in scope.

```
|-- programs/
|   |-- tokens/
|   |   |-- Cargo.toml
|   |   |-- Xargo.toml
|   |   |-- src/
|   |   |   |-- instructions/
|   |   |   |   |-- withdraw_tokens_fee.rs
|   |   |   |   |-- init_market.rs
|   |   |   |   |-- init_token.rs
|   |   |   |   |-- init_ata.rs
|   |   |   |   |-- process_completed_curve.rs
|   |   |   |   |-- mod.rs
|   |   |   |   |-- sell.rs
|   |   |   |   |-- buy.rs
|   |   |   |-- events.rs
|   |   |   |-- lib.rs
|   |   |   |-- state/
|   |   |   |   |-- market.rs
|   |   |   |   |-- bonding_curve.rs
|   |   |   |   |-- mod.rs
|   |   |   |-- errors.rs

```

## Steps to Run

We use Anchor. Testing and run are pretty much the standard for anchor.

We’re not sharing our keypair, so you will need to reset it to test the first time.

cd local-validator && ./run.sh
anchor reset
anchor build
anchor keys sync
anchor test --skip-local-validator
