# yield_generator Program — Developer Guide

This guide explains every component of the `yield_generator` Solana program:
its on-chain accounts, instructions, reward accrual math, yield direction toggle,
and the CPI surface it exports for `stake_v2`.

> **Codebase note:** This program runs on **Anchor v2** (`anchor-next`).
> If you are familiar with Anchor v1, see `../../ANCHOR_V2_MIGRATION_NOTES.md`
> for a detailed before/after comparison.

---

## Table of contents

1. [Repository layout](#1-repository-layout)
2. [On-chain accounts (state)](#2-on-chain-accounts-state)
3. [Constants](#3-constants)
4. [Error types](#4-error-types)
5. [Instructions — detailed walkthroughs](#5-instructions--detailed-walkthroughs)
   - [initialize](#initialize)
   - [deposit](#deposit)
   - [withdraw](#withdraw)
   - [set_yield_direction](#set_yield_direction)
6. [Reward accrual formula](#6-reward-accrual-formula)
7. [Yield direction — gain vs. loss mode](#7-yield-direction--gain-vs-loss-mode)
8. [CPI surface (for stake_v2)](#8-cpi-surface-for-stake_v2)
9. [PDA derivation reference](#9-pda-derivation-reference)
10. [Integration checklist](#10-integration-checklist)

---

## 1. Repository layout

```
programs/yield_generator/
├── Cargo.toml
└── src/
    ├── lib.rs               # Program entry-points + manual CPI module
    ├── constants.rs         # PDA seeds, APY_BPS, BPS_DENOMINATOR, SECONDS_PER_YEAR
    ├── state.rs             # YieldState, UserPosition
    ├── error.rs             # YieldError enum
    └── instructions/
        ├── mod.rs                 # Module re-exports
        ├── initialize.rs          # Create YieldState and yield vault
        ├── deposit.rs             # Accept SOL, create position
        ├── withdraw.rs            # Accrue rewards, return SOL, close position
        └── set_yield_direction.rs # Toggle gain / loss mode
```

### Why constants live in `constants.rs` (not `lib.rs`)

In the original v1 code all constants were declared inline in `lib.rs`.
Moving them to a dedicated module gives two benefits:

1. **Exported to callers** — after `pub use constants::*;` in `lib.rs`, any
   crate depending on `yield_generator` can write
   `yield_generator::STATE_SEED` instead of hard-coding `b"yield_state"`.
2. **Single source of truth** — instruction files import from `crate::constants`
   so every seed string is defined exactly once.

```
// Before (v1-era, in lib.rs)
pub const STATE_SEED: &[u8] = b"yield_state";  // scattered inline
pub const VAULT_SEED: &[u8] = b"yield_vault";
// ...

// After (v2, in constants.rs)
pub const STATE_SEED: &[u8] = b"yield_state";  // single authoritative location
// — re-exported via lib.rs: pub use constants::*
```

---

## 2. On-chain accounts (state)

### `YieldState` — global strategy state PDA

Seeds: `[b"yield_state"]`

```rust
pub struct YieldState {
    pub authority:                Address,  // set at init; controls set_yield_direction
    pub total_principal:          PodU64,   // sum of all active deposits
    pub total_yield_paid:         PodU64,   // cumulative positive-direction yield paid
    pub last_config_update_ts:    PodI64,   // unix timestamp of last config change
    pub apy_bps:                  PodU16,   // 1 000 = 10 % APY
    pub state_bump:               u8,
    pub vault_bump:               u8,
    pub yield_direction_positive: PodBool,  // true = gain, false = loss
}
```

### `UserPosition` — per-operator deposit record PDA

Seeds: `[b"user_position", operator_pubkey]`

```rust
pub struct UserPosition {
    pub owner:          Address,  // operator (depositor)
    pub principal:      PodU64,   // deposited lamports; must equal principal_returned on withdraw
    pub accrued_reward: PodU64,   // lamports accumulated since last_update_ts
    pub last_update_ts: PodI64,   // unix timestamp of last accrual
    pub bump:           u8,
}
```

> **Lifecycle:** Created by `deposit` with `init`; **closed** by `withdraw`
> (rent returned to operator).  Only one open position per operator at a time.

---

## 3. Constants

File: `src/constants.rs`

| Constant | Value | Purpose |
|----------|-------|---------|
| `STATE_SEED` | `b"yield_state"` | Seed for YieldState PDA |
| `VAULT_SEED` | `b"yield_vault"` | Seed for yield vault PDA |
| `POSITION_SEED` | `b"user_position"` | Seed prefix for per-operator position |
| `APY_BPS` | `1_000` | Default APY (10%) written at initialisation |
| `BPS_DENOMINATOR` | `10_000` | Divides BPS values to produce a fraction |
| `SECONDS_PER_YEAR` | `31_536_000` | Used in the reward formula |

---

## 4. Error types

File: `src/error.rs`

| Variant | When triggered |
|---------|---------------|
| `Unauthorized` | Signer does not match required authority |
| `InvalidAmount` | Zero amount supplied |
| `MathOverflow` | Any checked arithmetic overflows |
| `InsufficientPrincipal` | `position.principal` < requested withdrawal |
| `PartialWithdrawNotAllowed` | `principal_returned != position.principal` |
| `InvalidTime` | Clock moved backwards (now < last_update_ts) |
| `InsufficientVaultBalance` | Vault lamports < `total_out` |

---

## 5. Instructions — detailed walkthroughs

### `initialize`

**File:** `src/instructions/initialize.rs`  
**Signer:** `payer`

Creates the `YieldState` account and the yield vault PDA.

#### What it does

1. **Vault creation** — if `vault.data_len() == 0`, creates the vault as a
   zero-data system account via pinocchio `CreateAccount` with a PDA signer.
   If the vault already exists (idempotent path) the step is skipped.
2. **Sets state fields** — `authority = payer`, `apy_bps = APY_BPS` (10%),
   `yield_direction_positive = true`.

```rust
state.authority = *ctx.accounts.payer.address();
state.apy_bps   = PodU16::from(APY_BPS);           // 1 000 = 10%
state.yield_direction_positive = PodBool::from(true);
```

---

### `deposit`

**File:** `src/instructions/deposit.rs`  
**Signer:** `operator` (the fund manager)  
**Args:** `amount: u64`

Creates a new `UserPosition` and transfers SOL from the operator's source vault
to the yield vault.

#### Account layout

```
operator         — signer (fund manager)
state            — YieldState PDA, mutable
position         — NEW UserPosition PDA (init, not init_if_needed)
source_vault     — co-signer; must sign so system Transfer works
yield_vault      — receives the SOL
system_program
```

#### What it does

```
1. Require amount > 0
2. Read Clock timestamp (now)
3. Initialise position:
     owner          = operator
     principal      = amount
     accrued_reward = 0
     last_update_ts = now
4. Transfer amount lamports: source_vault → yield_vault
5. state.total_principal += amount
```

#### `init` vs `init_if_needed`

The `position` account uses `init` (not `init_if_needed`).  This means calling
`deposit` twice without an intervening `withdraw` will fail because the PDA
already exists.  This is intentional — it prevents double-deposits.

---

### `withdraw`

**File:** `src/instructions/withdraw.rs`  
**Signer:** `operator`  
**Args:** `principal_returned: u64`, `_yield_amount: u64`

Accrues time-weighted rewards, transfers `principal ± reward` to the destination
vault, and closes the `UserPosition`.

#### What it does

```
1. Require principal_returned > 0
2. Read Clock timestamp (now)
3. Validate position.owner == operator
4. Require principal_returned == position.principal   ← no partial withdraw
5. accrue_rewards(position, apy_bps, now)             ← updates position.accrued_reward
6. Compute total_out:
     if yield_direction_positive:  total_out = principal + accrued_reward
     else:                         total_out = principal.saturating_sub(accrued_reward)
7. Validate yield_vault.lamports >= total_out
8. Direct lamport mutation (v2 pattern):
     yield_vault.lamports     -= total_out
     destination_vault.lamports += total_out
9. Drain position accounting:
     position.principal      -= principal_returned
     position.accrued_reward -= reward_for_withdrawal
10. Update state:
     state.total_principal   -= principal_returned
     state.total_yield_paid  += reward_for_withdrawal  (only in positive direction)
```

> The `position` account has `close = operator` in the account constraint, so
> after the instruction its lamports (rent) are automatically returned to the
> operator by the runtime.

#### Why `_yield_amount` is ignored

The caller passes a `yield_amount` hint, but the program computes the actual
reward internally via `accrue_rewards`.  The argument exists only to maintain a
consistent instruction interface with what `stake_v2::withdraw_from_yield` sends.

---

### `set_yield_direction`

**File:** `src/instructions/set_yield_direction.rs`  
**Signer:** `authority` (matches `YieldState.authority`)

```rust
pub fn handler(ctx: &mut Context<SetYieldDirection>, is_positive: bool) -> Result<()> {
    require_keys_eq!(
        *ctx.accounts.authority.address(),
        ctx.accounts.state.authority,
        YieldError::Unauthorized
    );
    ctx.accounts.state.yield_direction_positive = PodBool::from(is_positive);
    Ok(())
}
```

**Effect:** Immediately changes how all future `withdraw` calls compute `total_out`.
Existing positions are unaffected until their next `withdraw`.

---

## 6. Reward accrual formula

```text
reward = principal × apy_bps × elapsed_seconds
         ────────────────────────────────────────
              BPS_DENOMINATOR × SECONDS_PER_YEAR
```

Where:
- `apy_bps = 1_000` (10%)
- `BPS_DENOMINATOR = 10_000`
- `SECONDS_PER_YEAR = 31_536_000`

**Example — 100 SOL staked for 15 days:**

```
elapsed = 15 × 24 × 60 × 60 = 1_296_000 seconds
reward  = 100_000_000_000 × 1_000 × 1_296_000
          ──────────────────────────────────────
               10_000 × 31_536_000
        = 100_000_000_000 × 1_296_000_000
          ──────────────────────────────────
               315_360_000_000
        ≈ 411_000_000 lamports  (≈ 0.411 SOL)
```

The formula uses `u128` intermediate arithmetic to prevent overflow:

```rust
let reward_u128 = (position.principal.get() as u128)
    .checked_mul(apy_bps as u128)?
    .checked_mul(elapsed as u128)?
    .checked_div(BPS_DENOMINATOR as u128)?
    .checked_div(SECONDS_PER_YEAR as u128)?;
```

---

## 7. Yield direction — gain vs. loss mode

The authority can toggle `yield_direction_positive` at any time.

| Mode | `total_out` formula | `total_yield_paid` updated? |
|------|--------------------|-----------------------------|
| Positive (`true`) | `principal + accrued_reward` | Yes |
| Negative (`false`) | `principal.saturating_sub(accrued_reward)` | No |

**Simulating market conditions:**
- Set `is_positive = true` before `withdraw` to simulate a bull market.
- Set `is_positive = false` before `withdraw` to simulate a loss scenario.

When `stake_v2` calls `withdraw_from_yield`, it observes the actual vault delta:

```
vault_delta = vault_after - vault_before
pnl = vault_delta - principal_returned   (signed)
```

In negative mode, `vault_delta < principal_returned` so `pnl < 0` and
`pool.total_sol` decreases — automatically depressing the `frankSOL` price.

---

## 8. CPI surface (for stake_v2)

When built with `features = ["cpi"]`, `yield_generator` exposes a typed CPI
surface in `lib.rs` under the `#[cfg(feature = "cpi")]` gate.

### Why it is hand-written

Anchor v2 does not auto-generate a CPI client the way v1 did.  The `cpi` module
is maintained manually and must stay in sync with the instruction account lists.

### Structure

```rust
// yield_generator/src/lib.rs (cpi feature)

pub mod cpi {
    pub mod accounts {
        pub struct Deposit<'a> {
            pub operator:      CpiHandle<'a>,  // readonly signer
            pub state:         CpiHandle<'a>,  // writable
            pub position:      CpiHandle<'a>,  // writable
            pub source_vault:  CpiHandle<'a>,  // readonly signer
            pub yield_vault:   CpiHandle<'a>,  // writable
            pub system_program:CpiHandle<'a>,  // readonly
        }
        // impl ToCpiAccounts<'a> for Deposit<'a> { ... }

        pub struct Withdraw<'a> { ... }
    }

    pub fn deposit<'a>(ctx: CpiContext<'a, accounts::Deposit<'a>>, amount: u64)
        -> Result<(), ProgramError> { ... }

    pub fn withdraw<'a>(ctx: CpiContext<'a, accounts::Withdraw<'a>>,
        principal_returned: u64, yield_amount: u64)
        -> Result<(), ProgramError> { ... }
}
```

### How stake_v2 calls it

```rust
// stake_v2/instructions/deploy_to_yield.rs
let cpi_accounts = yield_generator::cpi::accounts::Deposit {
    operator:       ctx.accounts.fund_manager.cpi_handle(),
    state:          ctx.accounts.yield_state.cpi_handle_mut(),
    position:       ctx.accounts.yield_position.cpi_handle_mut(),
    source_vault:   ctx.accounts.vault.cpi_handle(),         // vault must sign
    yield_vault:    ctx.accounts.yield_vault.cpi_handle_mut(),
    system_program: ctx.accounts.system_program.cpi_handle(),
};
let cpi_ctx = CpiContext::new(
    ctx.accounts.yield_generator_program.address(),
    cpi_accounts
).with_signer(&signer_seeds);  // vault PDA signer seeds
yield_generator::cpi::deposit(cpi_ctx, amount)?;
```

### Keeping the CPI in sync

If you add a new account to `deposit` or `withdraw` in `yield_generator`:
1. Update `cpi::accounts::Deposit` / `Withdraw` in `lib.rs`.
2. Update `ToCpiAccounts` impl to include the new handle.
3. Update the caller (`stake_v2`) to pass the new account.

---

## 9. PDA derivation reference

| Account | Seeds | Notes |
|---------|-------|-------|
| `YieldState` | `[b"yield_state"]` | Singleton per program deployment |
| Yield vault | `[b"yield_vault"]` | Zero-data SOL custody account |
| `UserPosition` | `[b"user_position", operator_pubkey]` | One per operator; closed on full withdraw |

### Off-chain derivation (TypeScript)

```typescript
import { PublicKey } from "@solana/web3.js";

const YIELD_PROGRAM_ID = new PublicKey("EKHVBmv9LcrRC64ryycKWphdkVdM1k2sVETbBvgSxCrf");

const [yieldState] = PublicKey.findProgramAddressSync(
  [Buffer.from("yield_state")],
  YIELD_PROGRAM_ID
);

const [yieldVault] = PublicKey.findProgramAddressSync(
  [Buffer.from("yield_vault")],
  YIELD_PROGRAM_ID
);

const [position] = PublicKey.findProgramAddressSync(
  [Buffer.from("user_position"), operatorPubkey.toBuffer()],
  YIELD_PROGRAM_ID
);
```

---

## 10. Integration checklist

When integrating `yield_generator` as a caller (e.g. adding a new strategy adapter):

- [ ] Build with `features = ["cpi"]` in your `Cargo.toml`.
- [ ] Call `initialize` once before any deposits.
- [ ] Derive PDAs using the exported constants: `yield_generator::STATE_SEED`,
      `yield_generator::VAULT_SEED`, `yield_generator::POSITION_SEED`.
- [ ] Always pass `source_vault` as a **signer** in the `deposit` CPI.
- [ ] Use `init` — not `init_if_needed` — awareness: each `deposit` needs a
      fresh position. Withdraw to close before re-depositing.
- [ ] In `withdraw`, pass `principal_returned = position.principal` exactly.
      Partial withdrawals are not supported and will return
      `PartialWithdrawNotAllowed`.
- [ ] Trust the actual vault delta for PnL; ignore the `_yield_amount` arg
      passed back through the CPI for your own accounting.
- [ ] Fund the yield vault externally (e.g. via `airdrop` in tests) when
      simulating positive yield so `total_out` does not exceed vault balance.
