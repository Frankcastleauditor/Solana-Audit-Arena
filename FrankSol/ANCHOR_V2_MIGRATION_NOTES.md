# stake_v2 — Anchor v2 Migration Notes

This document is a complete reference for the migration of `stake_v2` and
`yield_generator` from Anchor v1 to Anchor v2 (`anchor-next`).  It covers:

- every change made, with **before/after code snippets** for each,
- _why_ each change was made,
- the subsequent readability/organisation refactors applied on top,
- build/test status and known non-blocking warnings.

For day-to-day developer setup, see `ANCHOR_V2_SETUP_QUICKSTART.md`.  
For instruction-by-instruction walkthroughs, see the program-level guides:
- `programs/stake_v2/PROGRAM_GUIDE.md`
- `programs/yield_generator/PROGRAM_GUIDE.md`

---

## Table of contents

1. [Scope and decisions](#1-scope-and-decisions)
2. [Executive summary](#2-executive-summary)
3. [Section A — Anchor v1 → v2 changes](#section-a--anchor-v1--v2-changes)
   - [A1. Dependencies](#a1-dependencies)
   - [A2. Handler and context signatures](#a2-handler-and-context-signatures)
   - [A3. Key type: `Pubkey` → `Address`](#a3-key-type-pubkey--address)
   - [A4. State serialisation: Borsh → Pod](#a4-state-serialisation-borsh--pod)
   - [A5. SPL CPI conventions](#a5-spl-cpi-conventions)
   - [A6. Custom CPI module (`yield_generator`)](#a6-custom-cpi-module-yield_generator)
   - [A7. System ops and lamport transfers](#a7-system-ops-and-lamport-transfers)
   - [A8. Testing harness and host crates](#a8-testing-harness-and-host-crates)
4. [Section B — Code organisation refactors (post-migration)](#section-b--code-organisation-refactors-post-migration)
   - [B1. Extract `yield_generator` constants into `constants.rs`](#b1-extract-yield_generator-constants-into-constantsrs)
   - [B2. Move `FeeRecipientArg` from `admin.rs` to `state.rs`](#b2-move-feerecipientarg-from-adminrs-to-statersrs)
   - [B3. Comprehensive doc-comments on all source files](#b3-comprehensive-doc-comments-on-all-source-files)
5. [Section C — File-by-file changelog](#section-c--file-by-file-changelog)
6. [Build and test status](#build-and-test-status)
7. [Known non-blocking warnings](#known-non-blocking-warnings)
8. [Suggested follow-ups](#suggested-follow-ups)

---

## 1. Scope and decisions

| Decision | Choice made |
|----------|-------------|
| Layout strategy | In-place rewrite (no parallel branch) |
| State layout | Pod zero-copy (`PodU64`, `PodBool`, `#[repr(C)]`) |
| Dependency source | Git-based Anchor v2 (`branch = "anchor-next"`) |
| Vault payout path | Kept semantically aligned with v1 |
| Reproducibility | Pinning to specific `rev` is a follow-up action |

---

## 2. Executive summary

The codebase was migrated from Anchor v1's Borsh-first model to Anchor v2's
Pod-first, `pinocchio`-backed model.  Key changes:

- State accounts converted to zero-copy Pod layouts (`PodU64`, `PodU16`, `PodBool`, `PodI64`).
- Instruction handlers changed from `fn ix(ctx: Context<Ix>)` to `fn ix(ctx: &mut Context<Ix>)`.
- `Pubkey` replaced by `Address` throughout state and args.
- SPL and system CPIs rewritten for v2 handle-based conventions.
- A hand-written CPI module added to `yield_generator` (Anchor v2 has no auto-generation).
- Post-migration: constants extracted, types reorganised, doc-comments added.

After all changes, both programs compile and the full test suite passes.

---

## Section A — Anchor v1 → v2 changes

### A1. Dependencies

**Why:** Anchor v2 is not published on crates.io yet; it lives on the
`anchor-next` branch of the foundation repo.  The old stable `anchor-lang` /
`anchor-spl` crates must be replaced, and new support crates added.

**Before (v1):**
```toml
[dependencies]
anchor-lang = "1.0.1"
anchor-spl  = "1.0.1"
```

**After (v2):**
```toml
[dependencies]
anchor-lang-v2  = { git = "https://github.com/solana-foundation/anchor.git",
                    branch = "anchor-next", default-features = false,
                    features = ["alloc"] }
anchor-spl-v2   = { git = "https://github.com/solana-foundation/anchor.git",
                    branch = "anchor-next", default-features = false }
pinocchio        = { version = "0.11", default-features = false, features = ["copy"] }
pinocchio-system = "0.6"
bytemuck         = { version = "1", features = ["derive"] }
borsh            = { version = "1" }
wincode          = { version = "0.5", features = ["derive"] }
solana-program-log   = { version = "1.1", features = ["macro"] }
solana-program-error = { version = "3.0", features = ["borsh"] }
```

The `cpi` feature also changes — it now gates the manual CPI module rather than
triggering v1 auto-generation:

```toml
[features]
cpi = ["no-entrypoint"]   # v2: enables the hand-written cpi module
```

---

### A2. Handler and context signatures

**Why:** Anchor v2 expects a mutable context reference so that the framework
can apply exit-time validation and write back account changes.  The `'info`
lifetime annotation on account structs was removed because v2 handles lifetimes
differently.

**Before (v1):**
```rust
pub fn stake(ctx: Context<Stake>, amount_sol: u64, min_franksol_out: u64) -> Result<()> {
    // ctx is immutable
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    // ...
}
```

**After (v2):**
```rust
pub fn stake(ctx: &mut Context<Stake>, amount_sol: u64, min_franksol_out: u64) -> Result<()> {
    // ctx is &mut — v2 requirement
}

#[derive(Accounts)]
pub struct Stake {   // no 'info lifetime
    #[account(mut)]
    pub user: Signer,
    // ...
}
```

---

### A3. Key type: `Pubkey` → `Address`

**Why:** Anchor v2 (pinocchio-backed) uses `Address` (a re-export of
`pinocchio::pubkey::Pubkey`) as its canonical key type.  The Solana SDK `Pubkey`
is no longer imported via the anchor prelude.

**Before (v1):**
```rust
pub struct Pool {
    pub admin:        Pubkey,
    pub fund_manager: Pubkey,
    // ...
}

require_keys_eq!(ctx.accounts.admin.key(), pool.admin, StakeError::Unauthorized);
```

**After (v2):**
```rust
pub struct Pool {
    pub admin:        Address,
    pub fund_manager: Address,
    // ...
}

require_keys_eq!(
    *ctx.accounts.admin.address(),  // .address() returns &Address in v2
    pool.admin,
    StakeError::Unauthorized
);
```

---

### A4. State serialisation: Borsh → Pod

**Why:** Anchor v2 encourages zero-copy, Pod-compatible account layouts.
`PodU64`/`PodU16`/`PodBool`/`PodI64` are wrapper types that are byte-safe
(`Copy + Pod + Zeroable`).  Reading a field requires `.get()` and writing it
requires `PodX::from(value)`.  This avoids Borsh de/serialisation on every
account access, saving compute units.

**Before (v1):**
```rust
#[account]
pub struct Pool {
    pub admin:        Pubkey,
    pub total_sol:    u64,
    pub deployed_sol: u64,
    // ...
}

// Reading and writing are plain field access
pool.total_sol += amount_sol;
```

**After (v2):**
```rust
#[account]
#[repr(C)]
pub struct Pool {
    pub admin:        Address,
    pub total_sol:    PodU64,   // must call .get() to read, PodU64::from() to write
    pub deployed_sol: PodU64,
    // ...
}

// Reading: .get()
let current = pool.total_sol.get();
// Writing: PodU64::from(...)
pool.total_sol = PodU64::from(current + amount_sol);
```

The same pattern applies to:
- `PodU16` for `fee_bps`, `apy_bps`
- `PodBool` for `is_blacklisted`, `yield_direction_positive`
- `PodI64` for `last_update_ts`

---

### A5. SPL CPI conventions

**Why:** Anchor v2 replaces `to_account_info()` with typed CPI handles
(`cpi_handle()` / `cpi_handle_mut()`).  `CpiContext::new().with_signer()` is
still used but the account wiring uses the new handle API.

**Before (v1):**
```rust
// mint_to CPI
let cpi_accounts = MintTo {
    mint:      ctx.accounts.franksol_mint.to_account_info(),
    to:        ctx.accounts.user_franksol_ata.to_account_info(),
    authority: ctx.accounts.mint_authority.to_account_info(),
};
let cpi_ctx = CpiContext::new_with_signer(
    ctx.accounts.token_program.to_account_info(),
    cpi_accounts,
    signer_seeds,
);
token::mint_to(cpi_ctx, franksol_out)?;
```

**After (v2):**
```rust
// mint_to CPI — note cpi_handle_mut() and cpi_handle()
let mint_ctx = CpiContext::new(
    ctx.accounts.token_program.address(),
    token_cpi::accounts::MintTo {
        mint:      ctx.accounts.franksol_mint.cpi_handle_mut(),
        to:        ctx.accounts.user_franksol_ata.cpi_handle_mut(),
        authority: ctx.accounts.mint_authority.cpi_handle(),
    },
).with_signer(&signer_seeds);
token_cpi::mint_to(mint_ctx, franksol_out)?;
```

The distinction between `cpi_handle()` (read-only) and `cpi_handle_mut()`
(writable) maps directly to the account constraints the callee expects.

---

### A6. Custom CPI module (`yield_generator`)

**Why:** Anchor v1 could auto-generate a typed CPI client for any program that
exposed `#[program]` handlers.  Anchor v2 has no equivalent auto-generation —
callers must write the CPI module by hand.

**Before (v1 — auto-generated):**
```rust
// stake_v2/Cargo.toml
yield_generator = { path = "../yield_generator", features = ["cpi"] }

// In stake_v2 — auto-generated by anchor
use yield_generator::cpi::deposit;
deposit(cpi_ctx, amount)?;  // types were auto-generated
```

**After (v2 — manual module in yield_generator/src/lib.rs):**
```rust
// yield_generator/src/lib.rs (only compiled with features = ["cpi"])
#[cfg(feature = "cpi")]
pub mod cpi {
    pub mod accounts {
        pub struct Deposit<'a> {
            pub operator:       CpiHandle<'a>,
            pub state:          CpiHandle<'a>,
            pub position:       CpiHandle<'a>,
            pub source_vault:   CpiHandle<'a>,
            pub yield_vault:    CpiHandle<'a>,
            pub system_program: CpiHandle<'a>,
        }

        impl<'a> ToCpiAccounts<'a> for Deposit<'a> {
            fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
                vec![
                    InstructionAccount::readonly_signer(self.operator.address()),
                    InstructionAccount::writable(self.state.address()),
                    // ... in exact instruction account order
                ]
            }
            // ...
        }
    }

    pub fn deposit<'a>(
        ctx: CpiContext<'a, accounts::Deposit<'a>>,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let ix_data = crate::instruction::Deposit { amount }.data();
        ctx.invoke(&ix_data)
    }
}
```

**Why the account order matters:** `to_instruction_accounts()` must return
handles in exactly the same order as the `#[derive(Accounts)]` struct of the
callee instruction.  Getting this wrong causes silent account mismatches.

---

### A7. System ops and lamport transfers

**Why:** Anchor v2 / pinocchio replaces `solana_program::system_instruction`
and `invoke_signed` patterns with typed instruction builders from `pinocchio_system`.
Direct lamport mutation (without a CPI) is also used in `yield_generator::withdraw`.

**Before (v1 — system_instruction):**
```rust
use solana_program::system_instruction;
use solana_program::program::invoke_signed;

let ix = system_instruction::transfer(&from, &to, lamports);
invoke_signed(&ix, &[from_info, to_info, system_prog], &[seeds])?;
```

**After (v2 — pinocchio_system):**
```rust
// For vault creation:
use pinocchio_system::instructions::CreateAccount;
use pinocchio::cpi::{Seed, Signer as CpiSigner};

let vault_bump_bytes = [ctx.bumps.vault];
let seeds = [Seed::from(VAULT_SEED), Seed::from(&vault_bump_bytes[..])];
let signer = CpiSigner::from(&seeds);
CreateAccount {
    from:     ctx.accounts.admin.account(),
    to:       ctx.accounts.vault.account(),
    lamports: anchor_lang_v2::cpi::rent_exempt_lamports(0)?,
    space:    0,
    owner:    ctx.program_id,
}.invoke_signed(&[signer])?;

// For SOL transfer in stake:
use pinocchio_system::instructions::Transfer;
Transfer {
    from:     ctx.accounts.user.account(),
    to:       ctx.accounts.vault.account(),
    lamports: amount_sol,
}.invoke()?;

// For direct lamport mutation in yield_generator::withdraw (no CPI):
vault.set_lamports(vault.lamports().checked_sub(total_out)?);
destination.set_lamports(destination.lamports().checked_add(total_out)?);
```

---

### A8. Testing harness and host crates

**Why:** v2 ships modular, granular Solana crates (`solana-clock`,
`solana-instruction`, etc.) rather than a monolithic `solana-sdk`.  The test
files must import from these new paths.

**Before (v1):**
```rust
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signer::Signer,
    // ...
};
use solana_program_test::*;
```

**After (v2):**
```rust
use solana_clock::Clock;
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_pubkey::Pubkey;
use solana_rent::Rent;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use litesvm::LiteSVM;  // LiteSVM replaces BanksClient / program_test
```

`litesvm = "0.11"` is used for integration tests — it runs the Solana runtime
in-process without a validator, making tests faster and hermetic.

---

## Section B — Code organisation refactors (post-migration)

These changes were applied after the v1→v2 migration to improve readability
and developer experience.  They do not change runtime behaviour.

### B1. Extract `yield_generator` constants into `constants.rs`

**Why constants were in `lib.rs` (v1 legacy):** Anchor v1 programs commonly
placed constants inline in `lib.rs` next to the `declare_id!` macro.  There was
no reason to separate them.

**Problem:** Any crate that depends on `yield_generator` (i.e. `stake_v2`)
had to hard-code seed strings like `b"yield_state"` in its own source files.
If the seed ever changed, it had to be updated in two places.

**Fix:** Move constants to `src/constants.rs` and re-export via `pub use constants::*` in `lib.rs`.

**Before:**
```rust
// yield_generator/src/lib.rs
pub const STATE_SEED: &[u8]    = b"yield_state";
pub const VAULT_SEED: &[u8]    = b"yield_vault";
pub const POSITION_SEED: &[u8] = b"user_position";
pub const APY_BPS: u16         = 1_000;
pub const BPS_DENOMINATOR: u64 = 10_000;
pub const SECONDS_PER_YEAR: i64 = 365 * 24 * 60 * 60;
```

**After:**
```rust
// yield_generator/src/constants.rs  (NEW FILE)
pub const STATE_SEED: &[u8]     = b"yield_state";
pub const VAULT_SEED: &[u8]     = b"yield_vault";
pub const POSITION_SEED: &[u8]  = b"user_position";
pub const APY_BPS: u16          = 1_000;
pub const BPS_DENOMINATOR: u64  = 10_000;
pub const SECONDS_PER_YEAR: i64 = 365 * 24 * 60 * 60;

// yield_generator/src/lib.rs
pub mod constants;
// ...
pub use constants::*;  // re-exports STATE_SEED, VAULT_SEED, etc. to callers
```

Now any crate that depends on `yield_generator` can write:
```rust
use yield_generator::{STATE_SEED, VAULT_SEED, POSITION_SEED};
```
instead of duplicating string literals.

---

### B2. Move `FeeRecipientArg` from `admin.rs` to `state.rs`

**Why it was in `admin.rs` originally:** The struct was first created when
`set_fee_recipients` was written, so it was added in the same file as the
instruction handler.  This was convenient but wrong architecturally.

**Problem:** `FeeRecipientArg` is a **data type** — the wire-format twin of the
on-chain `FeeRecipient` struct.  Data types belong with other data types in
`state.rs`, not mixed into instruction logic.  Having it in `admin.rs` meant
any other instruction that might need it in the future would have to import from
the wrong module.

**Before:**
```rust
// admin.rs — instruction logic + a stray data type
use anchor_lang_v2::borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, ...)]
pub struct FeeRecipientArg {       // ← data type living in instruction file
    pub pubkey:  Address,
    pub fee_bps: u16,
}

pub fn set_fee_recipients(ctx: ..., slots: [FeeRecipientArg; 3]) -> Result<()> {
    // ...
}
```

**After:**
```rust
// state.rs — all data types here
use anchor_lang_v2::borsh::{BorshDeserialize, BorshSerialize};

pub struct FeeRecipient    { ... }  // on-chain Pod layout
pub struct FeeRecipientArg { ... }  // wire Borsh layout — twin of FeeRecipient

// admin.rs — only instruction logic
use crate::state::{FeeRecipient, FeeRecipientArg, Pool, UserPosition};
// — BorshSerialize/BorshDeserialize import removed from admin.rs
```

---

### B3. Comprehensive doc-comments on all source files

**Why:** The codebase had minimal inline documentation.  A new developer reading
the code had no immediate guidance on field semantics, function contracts, error
conditions, or the reasoning behind design choices.

**Pattern applied:** Module-level `//!` doc-comments explain the purpose and key
design decisions of each file.  Struct-level `///` doc-comments explain each
field.  Function-level `///` doc-comments explain contracts, error conditions,
and non-obvious behaviour.

**Examples:**

```rust
// constants.rs — before
pub const MAX_TOTAL_FEE_BPS: u16 = 500;

// constants.rs — after
/// Maximum allowed sum of all `fee_bps` values across the three fee slots
/// (500 bps = 5 %).  Enforced in `set_fee_recipients`.
pub const MAX_TOTAL_FEE_BPS: u16 = 500;
```

```rust
// utils.rs — before
pub fn sol_to_franksol(sol_in: u64, total_sol: u64, supply: u64) -> Result<u64> { ... }

// utils.rs — after
/// Converts a SOL amount to `frankSOL` using the pool's current exchange rate.
///
/// Formula (proportional share minting):
/// ```text
/// frankSOL_out = sol_in × supply / total_sol
/// ```
///
/// **Bootstrap case** (`supply == 0 || total_sol == 0`): the pool is empty so
/// the first depositor sets the price at 1:1, i.e. `frankSOL_out = sol_in`.
///
/// # Errors
/// - [`StakeError::InvalidAmount`] — `sol_in` is zero, or the result rounds to zero.
/// - [`StakeError::MathOverflow`] — intermediate multiplication overflowed.
pub fn sol_to_franksol(sol_in: u64, total_sol: u64, supply: u64) -> Result<u64> { ... }
```

```rust
// withdraw.rs — before
fn accrue_rewards(position: &mut Account<UserPosition>, apy_bps: u16, now: i64) -> Result<()> { ... }

// withdraw.rs — after
/// Accrues time-weighted rewards on `position` up to timestamp `now`.
///
/// Formula:
/// ```text
/// reward = principal × apy_bps × elapsed_seconds
///          ────────────────────────────────────────
///               BPS_DENOMINATOR × SECONDS_PER_YEAR
/// ```
fn accrue_rewards(position: &mut Account<UserPosition>, apy_bps: u16, now: i64) -> Result<()> { ... }
```

---

## Section C — File-by-file changelog

### `programs/yield_generator/Cargo.toml`
- `anchor-lang` → `anchor-lang-v2` (git branch).
- Added `pinocchio`, `pinocchio-system`.
- Added v2 support crates (`wincode`, `solana-program-log`, `solana-program-error`).
- Updated test-only Solana crates for v2 harness compatibility.
- Aligned `litesvm` to `0.11.0`.

### `programs/yield_generator/src/lib.rs`
- Converted to v2 prelude; handlers use `&mut Context<_>`.
- Added modular structure (`state`, `error`, `instructions`, `constants`).
- **Refactor (B1):** Removed inline constants; now `pub mod constants; pub use constants::*`.
- Added manual `cpi` module under `#[cfg(feature = "cpi")]`.
- Added module-level `//!` doc-comment.

### `programs/yield_generator/src/constants.rs` ← **NEW FILE (B1)**
- Extracted `STATE_SEED`, `VAULT_SEED`, `POSITION_SEED`, `APY_BPS`,
  `BPS_DENOMINATOR`, `SECONDS_PER_YEAR` from `lib.rs`.
- Added `///` doc-comments to each constant.

### `programs/yield_generator/src/state.rs`
- `YieldState` and `UserPosition` fields converted to Pod wrappers.
- Identity fields migrated to `Address`.
- `#[repr(C)]` added for zero-copy layout guarantee.
- Added `///` field-level doc-comments.

### `programs/yield_generator/src/error.rs`
- Moved to `anchor_lang_v2::prelude::*`.
- Added `//!` module doc-comment and `///` variant doc-comments.

### `programs/yield_generator/src/instructions/initialize.rs`
- v2 account/context signatures.
- Vault-create logic updated to pinocchio `CreateAccount`.
- State writes use Pod wrapper values.
- Added `//!` module doc-comment and `///` struct/handler doc-comments.

### `programs/yield_generator/src/instructions/deposit.rs`
- v2 account/context signatures.
- Pod field updates for principal/accounting.
- System transfer via pinocchio `Transfer`.
- Added `//!` module doc-comment explaining `init` vs `init_if_needed` choice.

### `programs/yield_generator/src/instructions/withdraw.rs`
- v2 account/context signatures.
- Reward and accounting updates Pod-based.
- Vault/destination lamport moves via direct mutation (v2 pattern).
- Added doc-comments to `handler` and `accrue_rewards` with the reward formula.

### `programs/yield_generator/src/instructions/set_yield_direction.rs`
- v2 account/context signatures.
- `Address` authority checks and `PodBool` state write.
- Added `//!` module doc-comment and `///` doc-comments.

### `programs/stake_v2/Cargo.toml`
- `anchor-lang`/`anchor-spl` → `anchor-lang-v2`/`anchor-spl-v2` (git branch).
- Added v2 support crates (`wincode`, `solana-program-log`, `solana-program-error`).
- Aligned `litesvm` to `0.11.0`.
- `cpi` feature updated to v2-compatible shape.

### `programs/stake_v2/src/lib.rs`
- Handler signatures migrated to `&mut Context<_>`.
- `FeeRecipientArg` arg type wired through from `state`.
- Added module-level `//!` doc-comment listing all sub-modules.

### `programs/stake_v2/src/constants.rs`
- Added `///` doc-comments to each constant explaining purpose.

### `programs/stake_v2/src/state.rs`
- `FeeRecipient`, `Pool`, `UserPosition` converted to Pod-compatible layout.
- `#[repr(C)]` added.
- **Refactor (B2):** `FeeRecipientArg` moved here from `admin.rs`; its Borsh
  import moved here too.
- Added comprehensive `///` field-level doc-comments to all three structs.

### `programs/stake_v2/src/error.rs`
- Moved to `anchor_lang_v2::prelude::*`.
- Added `//!` module doc-comment and `///` variant doc-comments.

### `programs/stake_v2/src/utils.rs`
- Retained core math helpers `sol_to_franksol`, `franksol_to_sol`,
  `checked_add_u64`, `checked_sub_u64`.
- Added `//!` module doc-comment explaining the u128 arithmetic strategy.
- Added full `///` doc-comments to each function with formula, bootstrap case,
  and error conditions.

### `programs/stake_v2/src/instructions.rs`
- Added `//!` module doc-comment.

### `programs/stake_v2/src/instructions/initialize.rs`
- Migrated to v2 account signatures and imports.
- Vault initialization via pinocchio `CreateAccount`.
- State writes use Pod wrappers.
- Added `//!` module doc-comment and `///` struct/handler doc-comments.

### `programs/stake_v2/src/instructions/stake.rs`
- Migrated account types and token program imports for v2.
- Pod-based state/account math updates.
- v2 CPI mint call using handle-based accounts.
- Added `//!` module doc-comment explaining the slippage mechanism.

### `programs/stake_v2/src/instructions/unstake.rs`
- Migrated to v2 account signatures and imports.
- Pod-based fee/state accounting updates.
- v2 token burn CPI and signer-seed updates.
- Added `//!` module doc-comment explaining fee distribution and the three
  always-required fee-recipient accounts.

### `programs/stake_v2/src/instructions/deploy_to_yield.rs`
- Migrated to v2 account/CPI shapes.
- Updated cross-program account typing and signer-seed handling.
- Added `//!` module doc-comment explaining the `init` position constraint
  and the vault-as-signer requirement.

### `programs/stake_v2/src/instructions/withdraw_from_yield.rs`
- Migrated to v2 account/CPI shapes.
- Updated yield CPI wiring and Pod-based pool accounting.
- Added `//!` module doc-comment with the PnL accounting formula.

### `programs/stake_v2/src/instructions/admin.rs`
- Migrated to v2 account signatures and token imports.
- **Refactor (B2):** `FeeRecipientArg` definition removed; now imported from
  `crate::state`. `BorshSerialize/BorshDeserialize` import removed.
- Freeze/thaw CPI flows updated to v2 conventions.
- Added `///` doc-comments to all four instruction contexts and handlers
  explaining authority requirements and side-effects.

### `programs/yield_generator/tests/test_yield_generator.rs`
- Updated imports to v2-compatible host crates.
- Adjusted assertions and harness expectations for migrated runtime/data flow.
- Maintained end-to-end scenario (initialize/deposit/time-advance/withdraw/close).

### `programs/stake_v2/tests/test_initialize.rs`
- Math tests retained and validated against migrated helper behaviour.

---

## Build and test status

```bash
# Validate compilation
cargo check --manifest-path stake_v2/programs/yield_generator/Cargo.toml
cargo check --manifest-path stake_v2/programs/stake_v2/Cargo.toml

# Run full workspace test suite
cargo test --manifest-path stake_v2/Cargo.toml

# Focused per-program tests
cargo test --manifest-path stake_v2/programs/stake_v2/Cargo.toml
cargo test --manifest-path stake_v2/programs/yield_generator/Cargo.toml
```

**Result:** workspace compiles, all tests pass (including LiteSVM integration test in `yield_generator`).

---

## Known non-blocking warnings

All warnings listed below are pre-existing and non-fatal.  They do not affect
correctness or test results.

| Warning | Origin | Notes |
|---------|--------|-------|
| `anchor-v2 idl: unable to classify seed expression` | Anchor IDL macro | Constant seed expressions not yet handled by IDL extractor |
| `unexpected_cfg` for `target_os = "solana"` | pinocchio macro expansion | Upstream issue in pinocchio; suppress with existing `[lints]` in `Cargo.toml` |
| `ambiguous glob re-exports` for `handler` | instruction module re-exports | Each instruction module exports a `handler` fn; name clashes on glob re-export. Does not affect runtime. |

---

## Suggested follow-ups

1. **Pin git dependencies to a specific `rev`** for build reproducibility.
   `anchor-next` moves quickly and a new commit may break compilation:
   ```toml
   anchor-lang-v2 = { git = "...", branch = "anchor-next", rev = "<sha>" }
   ```

2. **Replace glob re-exports** with explicit symbol exports to eliminate the
   `ambiguous_glob_reexports` warning:
   ```rust
   // Instead of: pub use stake::*;
   pub use stake::{Stake, handler as stake_handler};
   ```

3. **Evaluate `yield_generator::deposit` `init` vs `init_if_needed`** — the
   current `init` constraint means one position per operator lifetime.  If the
   protocol needs repeated deposits without a full withdraw cycle, the account
   constraint must change to `init_if_needed` with appropriate reset logic.

4. **Use exported constants in cross-program seed expressions** in
   `deploy_to_yield.rs` and `withdraw_from_yield.rs`.  Currently those files use
   string literals (`b"yield_state"`).  Since `yield_generator::STATE_SEED` is
   now exported, future Anchor versions that allow external constants in seed
   macros would enable using those directly.
