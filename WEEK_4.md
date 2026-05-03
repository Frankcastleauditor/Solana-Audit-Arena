# Week 4 — FrankSol (stake_v2 + yield_generator)

**Category**: Liquid Staking / Yield Strategy  
**Difficulty**: 🔴 Advanced  
**Submission deadline**: Sunday [DATE] 23:59 UTC

---

## Overview

This week targets a two-program system: **`stake_v2`** is a liquid staking protocol that lets users deposit SOL and receive `frankSOL` — a rebasing LST minted at the current pool exchange rate. A privileged fund manager can deploy pooled SOL into an external yield strategy (**`yield_generator`**) and withdraw it back with accrued rewards, which flow back to all `frankSOL` holders through the exchange rate. Fees are collected on unstake and distributed to up to three configurable recipients. Both programs run on **Anchor v2**.

The two programs interact via CPI: `stake_v2` calls `yield_generator::deposit` and `yield_generator::withdraw` through a manually implemented CPI module gated behind the `cpi` feature flag.

---

## Programs in Scope

| Program | Program ID |
|---------|-----------|
| `stake_v2` | `tsXqrLjaTSNeyCEMTBAHPZkyDvxRHBuv1t29f3rkZpz` |
| `yield_generator` | `EKHVBmv9LcrRC64ryycKWphdkVdM1k2sVETbBvgSxCrf` |

---

## Instructions

### stake_v2

| Instruction | Signer | Description |
|------------|--------|-------------|
| `initialize` | admin | Bootstraps the Pool PDA, frankSOL mint, and SOL vault. Sets initial fee recipient (treasury) at 500 bps. |
| `stake` | user | Deposits `amount_sol` lamports into the vault and mints `frankSOL` proportional to the current exchange rate. Accepts a `min_franksol_out` slippage guard. |
| `unstake` | user | Burns `franksol_in` tokens, redeems the equivalent SOL minus fees, and distributes fee splits to up to three recipients. Accepts a `min_sol_out` slippage guard. |
| `deploy_to_yield` | fund_manager | Transfers liquid SOL from the vault to `yield_generator` via CPI. Tracks deployed amount in `pool.deployed_sol`. |
| `withdraw_from_yield` | fund_manager | Calls `yield_generator::withdraw` via CPI and settles PnL: gains increase `total_sol`, losses decrease it. |
| `set_admin` | admin | Transfers admin role to a new address. |
| `set_fund_manager` | admin | Changes the fund manager address. |
| `set_fee_recipients` | admin | Configures up to three fee recipients with individual bps splits (capped at 500 bps total). |
| `set_user_blacklist` | admin | Flags or unflags a user as blacklisted; freezes or thaws their frankSOL ATA accordingly. |

### yield_generator

| Instruction | Signer | Description |
|------------|--------|-------------|
| `initialize` | payer | Creates the `YieldState` PDA and yield vault. Initializes APY at 10% (1,000 bps). |
| `deposit` | operator | Accepts SOL from a caller-controlled source vault and creates a `UserPosition` PDA tracking principal. |
| `withdraw` | operator | Accrues time-weighted rewards onto the position, then transfers `principal + reward` (or `principal - reward` in loss mode) back to the caller's destination vault. Closes the position. |
| `set_yield_direction` | authority | Toggles whether yield accrual adds to or subtracts from principal on withdrawal. |

---

## Accounts

### stake_v2

| Account | Seeds | Type | Description |
|---------|-------|------|-------------|
| `Pool` | `[b"pool"]` | PDA | Global pool state: roles, fee config, SOL/frankSOL accounting |
| `UserPosition` | `[b"user_position", user_pubkey]` | PDA | Per-user frankSOL balance, deposited SOL, blacklist flag |
| `frankSOL mint` | `[b"franksol_mint"]` | PDA (Mint) | The liquid staking token mint |
| `vault` | `[b"vault"]` | PDA (system account) | Holds all liquid SOL deposits |
| `mint_authority` | `[b"mint_auth"]` | PDA | Signing authority for mint/freeze/thaw CPIs |

### yield_generator

| Account | Seeds | Type | Description |
|---------|-------|------|-------------|
| `YieldState` | `[b"yield_state"]` | PDA | Global strategy state: authority, APY, principal tracking, yield direction |
| `UserPosition` | `[b"user_position", operator_pubkey]` | PDA | Per-operator position: principal, accrued reward, timestamps |
| `yield_vault` | `[b"yield_vault"]` | PDA (system account) | Holds all deposited SOL for the strategy |

---

## Key Mechanics to Understand

**frankSOL exchange rate** — The mint ratio is `sol_in / total_sol * franksol_supply` on stake and the inverse on unstake. As yield accrues, `total_sol` grows relative to `franksol_supply`, making each frankSOL redeemable for more SOL over time.

**PnL settlement** — When `withdraw_from_yield` executes, `vault_after - vault_before` is compared to `principal_returned`. The delta (positive or negative) is applied to `pool.total_sol`, which directly moves the frankSOL exchange rate.

**Yield accrual** — `yield_generator` computes rewards as `principal * apy_bps * elapsed_seconds / BPS_DENOMINATOR / SECONDS_PER_YEAR`. APY is fixed at 10%. Loss mode (`yield_direction_positive = false`) subtracts the same reward value from the principal on withdrawal.

**Fee distribution** — Fees are taken on `unstake` only, computed per recipient in bps of the raw SOL redemption amount. Transfers go directly from the vault PDA using CPI signer seeds.

---

## Scope

- All code in `programs/stake_v2/src/` is in scope
- All code in `programs/yield_generator/src/` is in scope
- The CPI boundary between the two programs is in scope — treat both sides as attack surface
- Focus on: logic bugs, access control, arithmetic, PDA security, CPI validation, exchange rate manipulation, fee distribution, blacklist bypass, yield direction abuse
- Out of scope: test files, Anchor framework internals, Solana runtime issues

---

## How to Submit

Submit each finding as a **separate GitHub Issue** using the [submission template](../README.md#submission-format).

Issue title format: `[Week 4] [Severity] Short descriptive title`

---

*Solana Audit Arena — by Frank Castle (@0xcastle_chain)*
