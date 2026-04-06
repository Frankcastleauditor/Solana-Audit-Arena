# Zenon — Bonding Curve Token Launchpad

**Program ID**: `SLAPUqHm76SGThDr4JUyRDVsGBmxwGkxicWwknkgH5c`

**Framework**: Anchor

---

## Overview

Zenon is an on-chain token launchpad that lets anyone permissionlessly launch a token with a **constant-product bonding curve** (x × y = k). Each token launched through Zenon gets its own `BondingCurve` PDA that tracks real and virtual reserves, and the curve automatically completes when a configurable `escape_amount` of tokens is sold. On completion, the accumulated SOL and remaining tokens flow to the admin-controlled treasury and escape fee destination.

The protocol supports a versioned `Market` PDA that acts as a global configuration layer — controlling trading fees, escape (graduation) fees, initial mint supply, and treasury destinations. Anyone can create a token against a live market; the admin controls market parameters.

---

## Architecture

```
Market (PDA [b"market", version_le])
 └── BondingCurve (PDA [b"bonding_curve", mint])
      └── BondingCurveATA (Token account owned by BondingCurve)
```

Each token launch is independent. A single `Market` can back many bonding curves simultaneously.

---

## Instructions

| Instruction | Caller | Description |
|-------------|--------|-------------|
| `initialize_market` | Admin (payer) | Create a new versioned `Market` PDA with protocol parameters |
| `update_market` | Admin (`market.authority`) | Update an existing market's parameters |
| `init_token` | Anyone | Launch a new token — creates the mint, `BondingCurve` PDA, and bonding curve ATA; mints the initial supply |
| `init_ata` | Anyone (payer) | Create an associated token account for a given mint and authority |
| `buy_tokens` | Anyone (buyer) | Buy tokens from the bonding curve by depositing SOL; triggers curve completion if `escape_amount` is reached |
| `sell_tokens` | Anyone (seller) | Sell tokens back to the bonding curve in exchange for SOL |
| `process_completed_curve` | Admin (`market.authority`) | After curve completion: transfer remaining tokens to admin ATA and distribute accumulated SOL between admin and escape fee treasury |
| `withdraw_tokens_fee` | Admin (`market.authority`) | After curve completion and cooldown: withdraw a token fee allocation from the bonding curve ATA to the `tokens_fee_treasury` |

---

## Accounts

| Account | Type | Description |
|---------|------|-------------|
| `Market` | PDA `[b"market", version_le_bytes]` | Global config: authority, initial mint supply, escape amount, escape fee, trading fee, treasury addresses |
| `BondingCurve` | PDA `[b"bonding_curve", mint]` | Per-token state: virtual/real SOL and token reserves, token supply, completion flag, cooldown timestamp, linked market |
| `BondingCurveATA` | ATA (authority = `BondingCurve`) | Holds the token supply for an active bonding curve |
| Mint | Signer on creation | SPL token mint; mint authority is renounced (set to `None`) after `init_token` |

---

## Roles

| Role | How Identified | Permissions |
|------|---------------|-------------|
| **Admin** | `market.authority` | Initialize markets (`initialize_market`); update market config (`update_market`); process completed curves (`process_completed_curve`); withdraw token fees (`withdraw_tokens_fee`) |
| **Anyone** | Any signer | Launch tokens (`init_token`); initialize ATAs (`init_ata`); buy tokens (`buy_tokens`); sell tokens (`sell_tokens`) |

### What each role explicitly cannot do

- **Anyone** cannot update market parameters — `update_market` requires a signer matching `market.authority`
- **Anyone** cannot call `process_completed_curve` or `withdraw_tokens_fee` — those are admin-gated
- **Admin** cannot prevent token launches or trades — the bonding curve is permissionless once a market exists
- **Admin** cannot withdraw from an active (non-completed) bonding curve — `process_completed_curve` and `withdraw_tokens_fee` both require `bonding_curve.completed == true`
- **`withdraw_tokens_fee`** is further gated by `tokens_fee_cooldown_timestamp` — admin must wait until the cooldown has elapsed

---

## Bonding Curve Mechanics

Zenon uses a **constant-product AMM** (`x × y = k`) via `spl_token_swap::curve::constant_product::swap` for price calculation.

Both real and virtual reserves are tracked:

- **Virtual reserves** — used as inputs to the AMM formula; initialized with configurable offsets (`sol_offset`, `token_offset`) to set the initial price
- **Real reserves** — track actual SOL and tokens held in the curve

**Buy flow:**
1. Compute `token_amount` out via constant-product swap on virtual reserves
2. Update both virtual and real reserves
3. Transfer SOL from buyer to `BondingCurve` PDA
4. Transfer tokens from `BondingCurveATA` to buyer ATA
5. Collect trading fee (in SOL) to `trading_fee_treasury`
6. Check if `token_supply - real_token_reserves >= escape_amount`; if so, mark `completed = true`

**Sell flow:**
1. Compute `sol_amount` out via constant-product swap on virtual reserves
2. Update both virtual and real reserves
3. Transfer tokens from seller ATA to `BondingCurveATA`
4. Collect trading fee (in SOL) from seller to `trading_fee_treasury`
5. Transfer SOL directly from `BondingCurve` lamports to seller

**Curve completion (`escape_amount` reached):**
- `process_completed_curve` transfers:
  - `real_token_reserves - tokens_fee_amount` → admin ATA
  - `real_sol_reserves - escape_fee` → admin (lamport transfer)
  - `escape_fee` → `escape_fee_treasury` (lamport transfer)
- `withdraw_tokens_fee` transfers `tokens_fee_amount` → `tokens_fee_treasury` ATA (after cooldown)

---

## State

### `Market`

```rust
pub struct Market {
    pub version: u16,
    pub authority: Pubkey,
    pub initial_mint: u64,
    pub escape_amount: u64,
    pub escape_fee_bps: u16,
    pub escape_fee_treasury: Pubkey,
    pub trading_fee_bps: u16,
    pub trading_fee_treasury: Pubkey,
    pub tokens_fee_amount: u64,
    pub tokens_fee_treasury: Pubkey,
}
```

### `BondingCurve`

```rust
pub struct BondingCurve {
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub token_supply: u64,
    pub completed: bool,
    pub tokens_fee_cooldown_timestamp: i64,
    pub market: Pubkey,
}
```

---

## Events

| Event | Emitted By | Fields |
|-------|-----------|--------|
| `InitTokenEvent` | `init_token` | `mint`, `timestamp`, virtual/real reserves |
| `TradeEvent` | `buy_tokens`, `sell_tokens` | `sol_amount`, `token_amount`, virtual/real reserves, `by`, `mint`, `is_buy`, `timestamp` |
| `BondingCurveCompletedEvent` | `buy_tokens` (on completion) | `mint`, virtual/real reserves, `timestamp` |

---

## Error Codes

| Error | Code | Meaning |
|-------|------|---------|
| `BondingCurveCompleted` | 0 | Cannot trade on a completed curve |
| `NotEnoughTokenReserves` | 1 | Insufficient token reserves |
| `TokenAmountZero` | 2 | Zero-amount trade attempt |
| `ExceededMaxSolAmount` | 3 | SOL input exceeds allowed max |
| `MinSolAmountNotMet` | 4 | SOL output below slippage floor |
| `MinTokenAmountNotMet` | 5 | Token output below slippage floor |
| `TransferError` | 6 | SPL transfer failed |
| `FeeBpsTooHigh` | 7 | Fee basis points exceed 10000 |
| `TokensBpsTooHigh` | 8 | `tokens_fee_amount >= escape_amount` |
| `EscapeAmountTooHigh` | 9 | `escape_amount > initial_mint` |
| `EscapeAmountZero` | 10 | `initial_mint == 0` |
| `BondingCurveNotCompleted` | 11 | Operation requires completed curve |
| `TokensFeeCooldown` | 12 | Token fee cooldown has not elapsed |

---

*Built with Anchor. Program ID: `SLAPUqHm76SGThDr4JUyRDVsGBmxwGkxicWwknkgH5c`*
