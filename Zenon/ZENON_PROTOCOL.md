# Zenon Protocol

## Overview

Zenon Protocol is a Solana-based token launchpad built with the Anchor framework. It enables permissionless token creation and trading through an automated bonding curve mechanism. Users can launch tokens, buy and sell them against the curve, and the protocol handles the full lifecycle from creation through bonding curve graduation.

## Architecture

The program is structured into three layers:

- **State** — on-chain account definitions (`Market`, `BondingCurve`)
- **Instructions** — executable program logic
- **Events** — emitted on-chain logs for indexing

---

## On-Chain Accounts

### `Market`

A versioned configuration account that defines the global parameters for a launchpad instance. Each market is identified by a `version` and derived as a PDA using `[b"market", version.to_le_bytes()]`.

| Field | Type | Description |
|---|---|---|
| `version` | `u16` | Unique market version identifier |
| `authority` | `Pubkey` | Admin allowed to update market params and withdraw funds |
| `initial_mint` | `u64` | Total token supply minted into the bonding curve at launch |
| `escape_amount` | `u64` | Number of tokens that must be sold before the curve graduates |
| `escape_fee_bps` | `u16` | Fee in basis points charged on SOL reserves at graduation |
| `escape_fee_treasury` | `Pubkey` | Recipient of the graduation fee |
| `trading_fee_bps` | `u16` | Fee in basis points charged on every buy/sell |
| `trading_fee_treasury` | `Pubkey` | Recipient of trading fees |
| `tokens_fee_amount` | `u64` | Token amount claimable by the treasury after graduation cooldown |
| `tokens_fee_treasury` | `Pubkey` | Recipient of the token fee |

### `BondingCurve`

A per-token account that tracks the state of a token's bonding curve. Derived as a PDA using `[b"bonding_curve", mint.key()]`.

| Field | Type | Description |
|---|---|---|
| `virtual_token_reserves` | `u64` | Virtual token reserves used for price calculation |
| `virtual_sol_reserves` | `u64` | Virtual SOL reserves used for price calculation |
| `real_token_reserves` | `u64` | Actual token balance held in the curve ATA |
| `real_sol_reserves` | `u64` | Actual SOL lamports held in the curve account |
| `token_supply` | `u64` | Initial token supply recorded at launch |
| `completed` | `bool` | Whether the curve has graduated |
| `tokens_fee_cooldown_timestamp` | `i64` | Unix timestamp after which the token fee can be claimed |
| `market` | `Pubkey` | The market this curve belongs to |

---

## Instructions

### Admin Instructions

#### `initialize_market`
Creates a new `Market` account with a given `version` and `MarketParams`. The market PDA is derived from the version, ensuring each version maps to exactly one market.

#### `update_market`
Updates the parameters of an existing market. Only callable by the current `authority`.

#### `process_completed_curve`
Called after a bonding curve has graduated (`completed = true`). Transfers remaining token reserves (minus the token fee reserve) to the admin's ATA, and distributes SOL reserves between the admin and the escape fee treasury according to `escape_fee_bps`.

#### `withdraw_tokens_fee`
Transfers the reserved token fee amount from the bonding curve ATA to the treasury ATA. Only callable by the market authority and only after the `tokens_fee_cooldown_timestamp` has passed.

### User Instructions

#### `init_token`
Launches a new token. This instruction:
1. Creates a new SPL `Mint`
2. Creates a `BondingCurve` PDA account
3. Creates an associated token account (ATA) owned by the bonding curve
4. Mints `initial_mint` tokens into the bonding curve ATA
5. Creates on-chain Metaplex metadata (name, symbol, URI)
6. Revokes mint authority so no further tokens can be minted

#### `init_ata`
Creates an associated token account for a given mint and authority. Useful for setting up buyer or seller ATAs before trading.

#### `buy_tokens`
Buys tokens from the bonding curve in exchange for SOL.

- Uses a constant-product AMM formula to compute the token output
- Checks slippage via `min_token_amount`
- Updates virtual and real reserves
- Transfers SOL from buyer to the bonding curve
- Transfers tokens from the bonding curve ATA to the buyer ATA
- Charges a `trading_fee_bps` fee on the SOL input, sent to `trading_fee_treasury`
- Marks the curve as `completed` if `escape_amount` of tokens have been sold

#### `sell_tokens`
Sells tokens back to the bonding curve in exchange for SOL.

- Uses a constant-product AMM formula to compute the SOL output
- Checks slippage via `min_sol_amount`
- Updates virtual and real reserves
- Transfers tokens from seller ATA to the bonding curve ATA
- Charges a `trading_fee_bps` fee on the SOL output, paid by the seller
- Transfers net SOL from the bonding curve to the seller

---

## Bonding Curve Mechanics

Zenon Protocol uses a **constant-product AMM** (x * y = k) via `spl_token_swap` for price discovery:

- At launch, virtual reserves are initialized with `token_offset` and `sol_offset` to define a starting price
- Real reserves track actual balances; virtual reserves track the price state
- As tokens are purchased, `real_sol_reserves` rises and `real_token_reserves` falls
- When the number of sold tokens reaches `escape_amount`, the curve is marked as graduated and trading halts

---

## Fee Structure

| Fee | When Charged | Denominator |
|---|---|---|
| Trading fee (`trading_fee_bps`) | On every buy and sell, on the SOL amount | 10,000 (bps) |
| Escape fee (`escape_fee_bps`) | On graduation, applied to total SOL reserves | 10,000 (bps) |
| Token fee (`tokens_fee_amount`) | After graduation cooldown, fixed token amount | N/A |

---

## Events

| Event | Emitted By | Description |
|---|---|---|
| `InitTokenEvent` | `init_token` | Token launched, includes initial reserve state |
| `TradeEvent` | `buy_tokens`, `sell_tokens` | Trade executed, includes amounts and reserve state |
| `BondingCurveCompletedEvent` | `buy_tokens` | Curve graduated, includes final reserve state |
