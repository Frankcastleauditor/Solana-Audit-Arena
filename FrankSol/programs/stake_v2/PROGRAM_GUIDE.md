# stake_v2 Program — Developer Guide

This guide explains every component of the `stake_v2` Solana program: its on-chain
accounts, instructions, math, and how it cross-program invokes `yield_generator`.
It is written as a reference for developers reading the code for the first time.

> **Codebase note:** This program runs on **Anchor v2** (`anchor-next`).
> If you are familiar with Anchor v1, see `../../ANCHOR_V2_MIGRATION_NOTES.md`
> for a detailed before/after comparison.

---

## Table of contents

1. [Repository layout](#1-repository-layout)
2. [On-chain accounts (state)](#2-on-chain-accounts-state)
3. [Constants](#3-constants)
4. [Math helpers (utils)](#4-math-helpers-utils)
5. [Error types](#5-error-types)
6. [Instructions — detailed walkthroughs](#6-instructions--detailed-walkthroughs)
   - [initialize](#initialize)
   - [stake](#stake)
   - [unstake](#unstake)
   - [deploy_to_yield](#deploy_to_yield)
   - [withdraw_from_yield](#withdraw_from_yield)
   - [Admin instructions](#admin-instructions)
7. [frankSOL exchange-rate mechanics](#7-franksol-exchange-rate-mechanics)
8. [Fee distribution mechanics](#8-fee-distribution-mechanics)
9. [CPI to yield_generator](#9-cpi-to-yield_generator)
10. [PDA derivation reference](#10-pda-derivation-reference)

---

## 1. Repository layout

```
programs/stake_v2/
├── Cargo.toml
└── src/
    ├── lib.rs               # Program entry-points (#[program] block)
    ├── constants.rs         # PDA seeds, fee caps, BPS denominator
    ├── state.rs             # Pool, UserPosition, FeeRecipient, FeeRecipientArg
    ├── error.rs             # StakeError enum
    ├── utils.rs             # Pure math: sol_to_franksol, franksol_to_sol, checked arithmetic
    ├── instructions.rs      # Module re-exports
    └── instructions/
        ├── initialize.rs          # One-time bootstrap
        ├── stake.rs               # Deposit SOL → mint frankSOL
        ├── unstake.rs             # Burn frankSOL → redeem SOL (with fees)
        ├── deploy_to_yield.rs     # Fund manager sends SOL to yield_generator
        ├── withdraw_from_yield.rs # Pull SOL back + settle PnL
        └── admin.rs               # Role rotation + fee config + blacklist
```

### Why is `FeeRecipientArg` in `state.rs` rather than `admin.rs`?

`FeeRecipientArg` is the wire-format (Borsh-serialisable) twin of the on-chain
`FeeRecipient` struct.  It is a **data type**, not instruction logic, so it lives
alongside the other data types in `state.rs`.  The admin instruction handler in
`admin.rs` imports it from there.

```rust
// state.rs — data types live here
pub struct FeeRecipient { ... }   // on-chain, Pod layout
pub struct FeeRecipientArg { ... } // wire, Borsh layout → converted before writing

// admin.rs — instruction logic
use crate::state::{FeeRecipient, FeeRecipientArg, Pool, UserPosition};
```

---

## 2. On-chain accounts (state)

### `Pool` — global state PDA

Seeds: `[b"pool"]`

```rust
pub struct Pool {
    pub admin:           Address,            // rotatable via set_admin
    pub fund_manager:    Address,            // rotatable via set_fund_manager
    pub franksol_mint:   Address,            // frankSOL SPL mint
    pub fee_recipients:  [FeeRecipient; 3],  // up to 3 fee slots; total bps ≤ 500
    pub total_sol:       PodU64,             // all SOL tracked (including deployed)
    pub deployed_sol:    PodU64,             // SOL currently in yield_generator
    pub franksol_supply: PodU64,             // mirror of SPL mint supply
    pub bump:            u8,
    pub vault_bump:      u8,
    pub mint_auth_bump:  u8,
}
```

**Key invariant:** `deployed_sol ≤ total_sol` at all times.
Liquid (unstakeable) SOL = `total_sol - deployed_sol`.

### `UserPosition` — per-staker PDA

Seeds: `[b"user_position", user_pubkey]`

```rust
pub struct UserPosition {
    pub owner:            Address,
    pub franksol_balance: PodU64,  // mirror of the user's ATA balance
    pub sol_deposited:    PodU64,  // cost basis — NOT decremented on unstake
    pub is_blacklisted:   PodBool, // blocks stake/unstake when true
    pub bump:             u8,
}
```

Created lazily via `init_if_needed` on the first `stake` call.

### `FeeRecipient` — fee slot (embedded in `Pool`)

```rust
pub struct FeeRecipient {
    pub pubkey:  Address,  // destination; Address::default() + 0 bps = inactive
    pub fee_bps: PodU16,   // fee for this slot in basis points
}
```

### `FeeRecipientArg` — wire type for `set_fee_recipients`

```rust
pub struct FeeRecipientArg {
    pub pubkey:  Address,
    pub fee_bps: u16,  // plain u16, not Pod — Borsh/wincode serialisable
}
```

Converted to `FeeRecipient` (Pod) before writing to the pool.

---

## 3. Constants

File: `src/constants.rs`

| Constant | Value | Purpose |
|----------|-------|---------|
| `POOL_SEED` | `b"pool"` | Seed for Pool PDA |
| `VAULT_SEED` | `b"vault"` | Seed for SOL vault PDA |
| `MINT_AUTH_SEED` | `b"mint_auth"` | Seed for mint/freeze authority PDA |
| `FRANKSOL_MINT_SEED` | `b"franksol_mint"` | Seed for frankSOL SPL mint |
| `USER_POSITION_SEED` | `b"user_position"` | Seed prefix for per-user position |
| `MAX_TOTAL_FEE_BPS` | `500` | Maximum sum of all fee slots (5%) |
| `BPS_DENOMINATOR` | `10_000` | Divide basis-point amounts by this |
| `PRECISION` | `1_000_000_000` | Fixed-point precision multiplier |

---

## 4. Math helpers (utils)

File: `src/utils.rs`

All helpers use `u128` intermediate arithmetic and `checked_*` operations to
prevent overflow.

### `sol_to_franksol`

```rust
// Bootstrap (pool empty): 1:1
if supply == 0 || total_sol == 0 { return Ok(sol_in); }

// Otherwise: proportional share minting
frankSOL_out = sol_in × supply / total_sol
```

### `franksol_to_sol`

```rust
// Proportional share redemption
sol_out = franksol_in × total_sol / supply
```

As `total_sol` grows (positive yield) each `frankSOL` redeems for more SOL.
As `total_sol` shrinks (loss) each `frankSOL` redeems for less SOL.

### `checked_add_u64` / `checked_sub_u64`

Thin wrappers over `u64::checked_add/sub` that map `None` to `StakeError::MathOverflow`.

---

## 5. Error types

File: `src/error.rs`

| Variant | Triggered by |
|---------|-------------|
| `Unauthorized` | Signer does not match expected authority |
| `InvalidAmount` | Zero amount or result rounds to zero |
| `MathOverflow` | Any checked arithmetic returns `None` |
| `InsufficientVaultBalance` | Vault has less liquid SOL than requested |
| `InsufficientDeployed` | `deployed_sol` < `principal_returned` |
| `SlippageExceeded` | Output is below caller's minimum threshold |
| `ZeroSupply` | Trying to redeem when `franksol_supply == 0` |
| `AlreadyInitialized` | Vault PDA already has data when `initialize` runs |
| `InvalidExternalProgram` | Supplied program account is not `yield_generator` |
| `UserBlacklisted` | User is blacklisted; stake/unstake blocked |
| `FeesExceedCap` | New fee config total would exceed 500 bps |
| `FeeRecipientMismatch` | `unstake` fee recipient key differs from pool slot |

---

## 6. Instructions — detailed walkthroughs

### `initialize`

**File:** `src/instructions/initialize.rs`  
**Signer:** `admin`

Creates the `Pool` PDA, vault PDA, and `frankSOL` mint in one transaction.

#### What it does step-by-step

1. **Idempotency guard** — checks `vault.data_len() == 0`. If the vault already
   exists it returns `AlreadyInitialized` immediately.
2. **Creates the vault PDA** — uses pinocchio `CreateAccount` with a signer seed
   (`[VAULT_SEED, vault_bump]`). The vault is a zero-data account owned by this
   program; it holds SOL purely as lamports.
3. **Writes `Pool` fields** — sets `admin`, `fund_manager`, the initial fee slot,
   and all counters to zero.

#### Initial fee table

```rust
pool.fee_recipients = [
    FeeRecipient {
        pubkey:  *ctx.accounts.treasury.address(),
        fee_bps: PodU16::from(MAX_TOTAL_FEE_BPS), // 500 bps = 5%
    },
    FeeRecipient::default(),  // inactive
    FeeRecipient::default(),  // inactive
];
```

Slot 0 is pre-seeded with the full 5% cap to the `treasury` key.  Run
`set_fee_recipients` to reconfigure.

---

### `stake`

**File:** `src/instructions/stake.rs`  
**Signer:** `user`  
**Args:** `amount_sol: u64`, `min_franksol_out: u64`

#### Flow

```
1. Require amount_sol > 0
2. Init (or validate) UserPosition
3. Require !is_blacklisted
4. frankSOL_out = sol_to_franksol(amount_sol, pool.total_sol, pool.franksol_supply)
5. Require frankSOL_out >= min_franksol_out   ← slippage guard
6. Transfer amount_sol lamports: user → vault  (pinocchio Transfer)
7. Mint frankSOL_out tokens: mint → user ATA   (SPL token CPI, signed by mint_authority PDA)
8. Update pool: total_sol += amount_sol, franksol_supply += frankSOL_out
9. Update position: franksol_balance += frankSOL_out, sol_deposited += amount_sol
```

#### First-stake bootstrap

When `pool.franksol_supply == 0` (fresh pool), `sol_to_franksol` returns
`sol_in` directly — the first depositor sets the price at 1 SOL = 1 frankSOL.

#### Signer seeds for CPI mint

The mint authority is a PDA and must sign the `MintTo` CPI:

```rust
let bump_bytes = [pool.mint_auth_bump];
let mint_seed_bytes: &[&[u8]] = &[MINT_AUTH_SEED, &bump_bytes];
let signer_seeds: &[&[&[u8]]] = &[mint_seed_bytes];
let mint_ctx = CpiContext::new(...).with_signer(&signer_seeds);
token_cpi::mint_to(mint_ctx, franksol_out)?;
```

---

### `unstake`

**File:** `src/instructions/unstake.rs`  
**Signer:** `user`  
**Args:** `franksol_in: u64`, `min_sol_out: u64`

#### Flow

```
1. Require franksol_in > 0
2. Validate user_position.owner == user
3. Require !is_blacklisted
4. sol_out = franksol_to_sol(franksol_in, pool.total_sol, pool.franksol_supply)
5. Require sol_out >= min_sol_out            ← slippage guard (gross, pre-fee)
6. Compute per-slot fees:
     fee_i = sol_out × fee_bps_i / BPS_DENOMINATOR
7. user_sol_out = sol_out − Σ fee_i
8. Require sol_out ≤ (total_sol − deployed_sol)  ← liquidity guard
9. Burn franksol_in tokens (SPL CPI, user signs)
10. Transfer user_sol_out: vault PDA → user
11. Transfer fee_i: vault PDA → fee_recipient_i  (for each active slot)
12. Update pool: total_sol -= sol_out, franksol_supply -= franksol_in
13. Update position: franksol_balance (saturating_sub)
```

#### Why three fee-recipient accounts are always required

The `unstake` instruction must have exactly three fee-recipient accounts matching
`pool.fee_recipients[0/1/2].pubkey`.  Even if a slot is inactive (0 bps, default
pubkey) the account must still be present and must match.  This keeps the account
validation constraint simple and prevents the program from having variable-length
account lists.

```rust
#[account(mut, address = pool.fee_recipients[0].pubkey @ StakeError::FeeRecipientMismatch)]
pub fee_recipient_0: UncheckedAccount,
```

---

### `deploy_to_yield`

**File:** `src/instructions/deploy_to_yield.rs`  
**Signer:** `fund_manager`  
**Args:** `amount: u64`

Calls `yield_generator::deposit` via CPI to move liquid SOL from the vault into
the strategy.

#### Liquidity check before CPI

```rust
let liquid_sol = checked_sub_u64(pool.total_sol.get(), pool.deployed_sol.get())?;
require!(amount <= liquid_sol, StakeError::InsufficientVaultBalance);
```

#### CPI construction

The vault PDA must sign the CPI because `yield_generator::deposit` requires the
source vault to co-sign the system `Transfer`:

```rust
let vault_bump = [pool.vault_bump];
let vault_seed_bytes: &[&[u8]] = &[VAULT_SEED, &vault_bump];
let signer_seeds: &[&[&[u8]]] = &[vault_seed_bytes];
let cpi_ctx = CpiContext::new(..., cpi_accounts).with_signer(&signer_seeds);
yield_generator::cpi::deposit(cpi_ctx, amount)?;
```

After CPI success: `pool.deployed_sol += amount`.

> **Note:** The `yield_generator::deposit` instruction uses `init` (not
> `init_if_needed`) for the `position` account.  Each `deploy_to_yield` call
> requires a fresh position PDA.  The fund manager must call
> `withdraw_from_yield` to close the existing position before re-deploying.

---

### `withdraw_from_yield`

**File:** `src/instructions/withdraw_from_yield.rs`  
**Signer:** `fund_manager`  
**Args:** `principal_returned: u64`, `yield_amount: u64`

Calls `yield_generator::withdraw` via CPI and settles the actual vault delta.

#### PnL accounting (trust the chain, not the caller)

The handler reads the vault balance **before** and **after** the CPI:

```rust
let vault_before = ctx.accounts.vault.lamports();

// ... CPI to yield_generator::withdraw ...

let vault_after = ctx.accounts.vault.lamports();
let total_return = checked_sub_u64(vault_after, vault_before)?;
let pnl_i128 = (total_return as i128) - (principal_returned as i128);

if pnl_i128 >= 0 {
    // Yield: increase total_sol → frankSOL price appreciates
    pool.total_sol += u64::try_from(pnl_i128)?;
} else {
    // Loss: decrease total_sol → frankSOL price depreciates
    let loss = u64::try_from(-pnl_i128)?;
    pool.total_sol -= loss;
}
```

The `yield_amount` argument is passed through to the CPI but the pool never uses
it for its own accounting — it only trusts the actual on-chain lamport delta.

---

### Admin instructions

**File:** `src/instructions/admin.rs`  
All admin instructions require the **current** `pool.admin` to sign.

#### `set_admin`

Rotates the admin key.  After this call the old admin has no further privileges.

```rust
pub fn set_admin(ctx: &mut Context<SetAdmin>, new_admin: Address) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    require_keys_eq!(*ctx.accounts.admin.address(), pool.admin, StakeError::Unauthorized);
    pool.admin = new_admin;
    Ok(())
}
```

#### `set_fund_manager`

Rotates the fund manager.  Old fund manager can no longer call
`deploy_to_yield` / `withdraw_from_yield`.

#### `set_fee_recipients`

Replaces all three fee slots atomically.  The total `fee_bps` across all slots
must not exceed `MAX_TOTAL_FEE_BPS`:

```rust
let total_bps = slots.iter().try_fold(0u16, |acc, slot| {
    acc.checked_add(slot.fee_bps).ok_or(StakeError::MathOverflow)
})?;
require!(total_bps <= MAX_TOTAL_FEE_BPS, StakeError::FeesExceedCap);
```

To deactivate a slot, pass `fee_bps: 0` and `pubkey: Address::default()`.

#### `set_user_blacklist`

Toggles `UserPosition.is_blacklisted` and freezes/thaws the user's `frankSOL`
ATA via CPI to the token program:

```rust
if is_blacklisted {
    token_cpi::freeze_account(...)  // ATA frozen — user cannot transfer
} else {
    token_cpi::thaw_account(...)    // ATA restored
}
```

The mint authority PDA signs with `[MINT_AUTH_SEED, mint_auth_bump]`.

---

## 7. frankSOL exchange-rate mechanics

`frankSOL` is a proportional share token: each token represents a fractional
ownership of the SOL pool.

```
price = total_sol / franksol_supply   (in lamports per token)
```

**On stake:** the user gets `sol_in × supply / total_sol` tokens.  
**On unstake:** the user gets `franksol_in × total_sol / supply` lamports.

| Event | Effect on rate |
|-------|---------------|
| Stake | No change (both `total_sol` and `supply` increase proportionally) |
| Unstake (no yield) | No change (both decrease proportionally) |
| `withdraw_from_yield` with positive PnL | `total_sol` increases, `supply` unchanged → rate increases |
| `withdraw_from_yield` with negative PnL | `total_sol` decreases → rate decreases |

This is identical in concept to liquid staking tokens like stSOL or mSOL.

---

## 8. Fee distribution mechanics

Fees are collected on every `unstake` and split across the three `Pool.fee_recipients` slots.

```
fee_i        = sol_out × fee_bps_i / BPS_DENOMINATOR
total_fee    = Σ fee_i   (over active slots only)
user_gets    = sol_out - total_fee
```

**Example:** 1 SOL redemption (`sol_out = 1_000_000_000 lamports`), single
treasury at 2% (200 bps):

```
fee_0 = 1_000_000_000 × 200 / 10_000 = 20_000_000 lamports (0.02 SOL)
user  = 1_000_000_000 - 20_000_000   = 980_000_000 lamports (0.98 SOL)
```

**Client requirement:** Always pass the three fee recipient accounts in account
order `[fee_recipient_0, fee_recipient_1, fee_recipient_2]` with the keys from
`pool.fee_recipients[0/1/2].pubkey`.  Fetch the `Pool` account first, then
construct the transaction.

---

## 9. CPI to yield_generator

`deploy_to_yield` and `withdraw_from_yield` call into `yield_generator` using
the typed CPI surface it exposes under the `cpi` feature:

```toml
# stake_v2/Cargo.toml
yield_generator = { path = "../yield_generator", features = ["cpi"] }
```

The CPI module in `yield_generator/src/lib.rs` (only compiled with `features = ["cpi"]`)
defines:

- `cpi::accounts::Deposit` — struct of `CpiHandle` fields matching the `deposit`
  account list in order.
- `cpi::accounts::Withdraw` — same for `withdraw`.
- `cpi::deposit(ctx, amount)` — serialises the discriminator + args and calls
  `ctx.invoke(...)`.
- `cpi::withdraw(ctx, principal_returned, yield_amount)` — same.

This is the **Anchor v2 manual CPI pattern**: there is no auto-generated client
surface.  You maintain the account list and instruction data yourself.

---

## 10. PDA derivation reference

| Account | Seeds | Program |
|---------|-------|---------|
| `Pool` | `[b"pool"]` | stake_v2 |
| Vault | `[b"vault"]` | stake_v2 |
| Mint authority | `[b"mint_auth"]` | stake_v2 |
| `frankSOL` mint | `[b"franksol_mint"]` | stake_v2 |
| `UserPosition` | `[b"user_position", user_pubkey]` | stake_v2 |
| `YieldState` | `[b"yield_state"]` | yield_generator |
| Yield vault | `[b"yield_vault"]` | yield_generator |
| Yield position | `[b"user_position", operator_pubkey]` | yield_generator |

### Deriving PDAs off-chain (TypeScript)

```typescript
import { PublicKey } from "@solana/web3.js";

const STAKE_PROGRAM_ID  = new PublicKey("tsXqrLjaTSNeyCEMTBAHPZkyDvxRHBuv1t29f3rkZpz");
const YIELD_PROGRAM_ID  = new PublicKey("EKHVBmv9LcrRC64ryycKWphdkVdM1k2sVETbBvgSxCrf");

const [pool]         = PublicKey.findProgramAddressSync([Buffer.from("pool")],           STAKE_PROGRAM_ID);
const [vault]        = PublicKey.findProgramAddressSync([Buffer.from("vault")],          STAKE_PROGRAM_ID);
const [mintAuth]     = PublicKey.findProgramAddressSync([Buffer.from("mint_auth")],      STAKE_PROGRAM_ID);
const [franksolMint] = PublicKey.findProgramAddressSync([Buffer.from("franksol_mint")],  STAKE_PROGRAM_ID);

const [userPos] = PublicKey.findProgramAddressSync(
  [Buffer.from("user_position"), userPubkey.toBuffer()],
  STAKE_PROGRAM_ID
);

const [yieldState] = PublicKey.findProgramAddressSync([Buffer.from("yield_state")], YIELD_PROGRAM_ID);
const [yieldVault] = PublicKey.findProgramAddressSync([Buffer.from("yield_vault")], YIELD_PROGRAM_ID);
```

---

## Common gotchas

1. **`deploy_to_yield` expects a fresh position PDA.** `yield_generator::deposit`
   uses `init`, so calling `deploy_to_yield` twice without a `withdraw_from_yield`
   in between will fail.

2. **`unstake` requires all three fee-recipient accounts**, even inactive ones.
   Pass `Address::default()` for inactive slots (it must still match the stored
   value).

3. **`min_sol_out` in `unstake` is gross (pre-fee).** The user actually receives
   `min_sol_out - fees`.  Factor this into your slippage calculation.

4. **`frankSOL` price is only updated when `withdraw_from_yield` is called.**
   During the time SOL is deployed, `total_sol` does not change — the price
   update happens when the fund manager brings the capital back.
