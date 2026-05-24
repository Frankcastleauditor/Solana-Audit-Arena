# Security Audit: FrankSol Protocol
04.05.26 - 11.05.26

## Overview
_Framework: Anchor v2 (Pinocchio runtime) · Native SOL · SPL Token_

The audited protocol is **FrankSol**, a two-program liquid staking system on Solana. Users deposit native SOL into `stake_v2` and receive `frankSOL`, a rebasing LST minted at the current pool exchange rate. A privileged fund manager can deploy pooled SOL into an external yield strategy (`yield_generator`) and withdraw it back with accrued rewards, which flow to all `frankSOL` holders through the exchange rate. Fees on `unstake` are split across up to three configurable recipients.

This audit was performed as part of [Solana Audit Arena — Week 4](https://github.com/Frankcastleauditor/Solana-Audit-Arena), a weekly security competition organized by Frank Castle.

## Scope
All code in:
- `programs/stake_v2/src/`
- `programs/yield_generator/src/`
- The CPI boundary between the two programs

## Architecture Summary

The system is composed of two on-chain programs that interact via CPI. The CPI is hand-rolled in a `cpi` module gated behind a feature flag rather than going through Anchor's generated CPI client.

| Program | Program ID |
|---------|-----------|
| `stake_v2` | `tsXqrLjaTSNeyCEMTBAHPZkyDvxRHBuv1t29f3rkZpz` |
| `yield_generator` | `EKHVBmv9LcrRC64ryycKWphdkVdM1k2sVETbBvgSxCrf` |

### stake_v2 — liquid staking
- A user calls `stake(amount_sol, min_franksol_out)` to deposit SOL and receive `frankSOL` at the current exchange rate. The mint formula is `franksol_out = sol_in × franksol_supply / total_sol`. For the first depositor the ratio is 1:1.
- `unstake(franksol_in, min_sol_out)` burns `frankSOL`, computes the gross SOL redemption `sol_out = franksol_in × total_sol / franksol_supply`, deducts up to three configurable fees (each ≤ 500 bps, total capped), and pays the net to the user.
- Pool state is held in a single `Pool` PDA seeded by `[b"pool"]`. SOL is held in a system-account vault PDA seeded by `[b"vault"]`. `frankSOL` is a native SPL Mint PDA.
- Each user has a `UserPosition` PDA seeded by `[b"user_position", user_pubkey]` tracking deposited SOL, `frankSOL` balance (mirror), and a blacklist flag.

### yield_generator — external strategy
- The fund manager calls `stake_v2::deploy_to_yield(amount)`, which transfers SOL from the stake vault to the yield vault via CPI and opens a `UserPosition` in `yield_generator` keyed by the operator pubkey.
- Rewards accrue continuously as `principal × apy_bps × elapsed / (BPS_DENOMINATOR × SECONDS_PER_YEAR)` with `apy_bps = 1000` (10% fixed).
- `stake_v2::withdraw_from_yield` calls `yield_generator::withdraw` via CPI, observes `vault_after − vault_before`, subtracts the returned `principal`, and applies the resulting PnL delta to `pool.total_sol` — moving the `frankSOL` exchange rate.
- The yield direction can be flipped from positive to negative by `YieldState.authority`; in loss mode the same reward formula is subtracted from principal on withdrawal.

## Findings Summary
| ID | Severity | Title |
|----|----------|-------|
| F-01 | Critical | Missing `fund_manager` authorization in `deploy_to_yield` (V1) |
| F-02 | Critical | Inverted PnL accounting in `withdraw_from_yield` (V3) |
| F-03 | Critical | Missing `yield_generator_program` address constraint in `withdraw_from_yield` (V5) |
| F-04 | Critical | Division before multiplication in `sol_to_franksol` (V8) |
| F-05 | High | `fee_recipient_2` constraint targets wrong pool index (V7) |
| F-06 | High | Missing `token::authority = user` on `user_franksol_ata` in `stake` (V4) |
| F-07 | Medium | Slippage guard checks gross `sol_out` instead of net `user_sol_out` (V2) |
| F-08 | Medium | `StakeEvent` emitted with stale pool state before update (V6) |
| F-09 | Medium | `source_vault` passed as readonly_signer in `deploy_to_yield` CPI metas |
| F-10 | Medium | Cross-program `Account<T>` deserialization fails owner check in `withdraw_from_yield` |
| F-11 | Medium | Vault PDA prefunding bricks `initialize` for both programs |
| F-12 | Medium | Permissionless `yield_generator::initialize` lets first caller capture `YieldState.authority` |
| F-13 | Medium | Permissionless `stake_v2::initialize` lets first caller capture `Pool.admin` |
| F-14 | Medium | `yield_generator::withdraw` accepts arbitrary `destination_vault` |
| F-15 | Medium | `yield_generator` pays APY from shared vault without solvency check |
| F-16 | Medium | `operator` marked readonly_signer in `withdraw_from_yield` CPI metas |
| F-17 | Low | `set_yield_direction` doesn't update `last_config_update_ts` |
| F-18 | Low | `set_admin` accepts `Pubkey::default()` and permanently locks admin controls |
| F-19 | Low | `set_fee_recipients` permits duplicate active recipients, bricking `unstake` |
| F-20 | Low | `frankSOL` holders without a `UserPosition` cannot `unstake` |
| F-21 | Low | `set_fund_manager` allows rotation while `deployed_sol > 0`, deadlocking `withdraw_from_yield` |
| F-22 | Low | `UserPosition` accounts are never closed (rent never recovered) |
| F-23 | Low | `Treasury` declared `UncheckedAccount` with no address validation |
| F-24 | Low | Permissionless `yield_generator::deposit` |
| F-25 | Low | `set_user_blacklist` cannot preemptively blacklist a user without a `UserPosition` |
| F-26 | Low | Yield direction change applied retroactively to in-flight positions |

## Detailed Findings

### [F-01] Missing `fund_manager` authorization in `deploy_to_yield` (V1)
`deploy_to_yield` is gated only by `fund_manager: Signer` in the account struct, which enforces that *some* signer is present but not *which* one. The handler-side `require_keys_eq!(fund_manager, pool.fund_manager, ...)` has been removed, so any keypair can pass as `fund_manager` and trigger the CPI that drains liquid SOL from the stake vault to `yield_generator`.

#### Attack vector
1. Attacker generates any keypair `K` and constructs the `DeployToYield` instruction with `K` as `fund_manager` and `amount = pool.total_sol − pool.deployed_sol`.
2. The CPI fires; the stake vault transfers all liquid SOL into `yield_vault`. The `yield_generator::deposit` opens a `UserPosition` keyed by **the attacker's pubkey** (`[b"user_position", K]`).
3. Because the position is owned by the attacker, they can call `yield_generator::withdraw` directly, routing the principal (plus any accrued reward) to a destination vault they control — bypassing `stake_v2`'s `withdraw_from_yield` entirely.
4. Even without the direct withdraw path, the deploy alone is a permanent DoS: `unstake` reverts with `InsufficientVaultBalance` until the legitimate admin rotates the fund manager and a real withdrawal recovers the funds.

#### Recommended fix
Restore the identity check at the top of the handler, before any state mutation:
```rust
require_keys_eq!(
    *ctx.accounts.fund_manager.address(),
    pool.fund_manager,
    StakeError::Unauthorized
);
```

### [F-02] Inverted PnL accounting in `withdraw_from_yield` (V3)
Both branches in the PnL settlement of `withdraw_from_yield.rs:76-82` are inverted. A positive yield (gain) is **subtracted** from `pool.total_sol`, and a loss is **added** to it. The vault's actual lamport balance and `pool.total_sol` diverge on every yield cycle, in opposite directions for the two failure modes.

#### Attack vector / failure modes
- **Honest yield destroys exchange rate.** Pool starts with `total_sol = 1,000`, vault holds 1,000 SOL. Fund manager deploys 900 SOL and withdraws after 10% yield accrual. `total_return = 990`, `pnl = +90`. Bug applies: `total_sol = 1,000 − 90 = 910`. Vault actual is 1,090. Every redeemer now gets a worse exchange rate than the vault can pay, and across repeated yield cycles the gap grows until `total_sol` underflows on the next gain.
- **Loss mode inflates exchange rate.** With `yield_direction_positive = false`, every "loss" is added to `total_sol`. An attacker who has acquired `frankSOL` cheaply can wait for the operator to toggle direction (or for the operator to be the attacker), trigger a withdrawal, and then `unstake` against the inflated `total_sol` — receiving more SOL than the vault should pay until the vault is drained (a classic bank-run).

#### Recommended fix
Swap the operations in both branches:
```rust
if pnl_i128 >= 0 {
    let gain = u64::try_from(pnl_i128)?;
    pool.total_sol = PodU64::from(checked_add_u64(pool.total_sol.get(), gain)?);
} else {
    let loss = u64::try_from(-pnl_i128)?;
    pool.total_sol = PodU64::from(checked_sub_u64(pool.total_sol.get(), loss)?);
}
```

### [F-03] Missing `yield_generator_program` address constraint in `withdraw_from_yield` (V5)
`withdraw_from_yield` declares `yield_generator_program` as a bare `UncheckedAccount` with no `#[account(address = yield_generator::id())]` constraint and no equivalent runtime check. `deploy_to_yield` pins the program correctly, creating a misleading appearance of coverage. Because `yield_state` and `yield_vault` are validated only via `seeds::program = yield_generator_program.address()`, all three accounts can be substituted in lockstep with a malicious program.

#### Attack vector
1. Attacker (or compromised fund manager) deploys a fake program with the same instruction discriminator for `withdraw` whose handler returns `Ok` without moving any SOL.
2. Calls `withdraw_from_yield` with the fake program as `yield_generator_program`. Seeds derive consistently against the fake program ID, so `yield_state` and `yield_vault` validation passes.
3. CPI succeeds. `vault_before == vault_after`, so `total_return = 0` and `pnl_i128 = −principal_returned`.
4. `pool.deployed_sol −= principal_returned`. With F-02 fixed, `pool.total_sol −= principal_returned` as well, writing the entire deployed amount off the books while no SOL ever returned to the stake vault. With F-02 still present, the loss branch (now adding) makes the accounting even more divergent.

#### Recommended fix
Restore the address constraint:
```rust
#[account(address = yield_generator::id())]
pub yield_generator_program: UncheckedAccount,
```

### [F-04] Division before multiplication in `sol_to_franksol` (V8)
`sol_to_franksol` in `utils.rs:20-24` computes the mint amount as `(sol_in / total_sol) × supply` instead of `(sol_in × supply) / total_sol`. In any realistic pool `sol_in < total_sol`, so the intermediate division truncates to zero, the final `out == 0` guard fires, and the `stake` instruction reverts with `InvalidAmount`. The inverse `franksol_to_sol` is implemented correctly (multiply-then-divide).

#### Attack vector / impact
The protocol is unusable in production. After the first non-trivial deposit, `pool.total_sol > sol_in` for all subsequent stakers; every call reverts. TVL is frozen at whatever the first depositor put in. The bug also passes any unit test that runs `stake` against a fresh empty pool (where `total_sol == 0` takes a different branch returning `sol_in`), so it can survive shallow test coverage and only surface under realistic integration tests with live yield.

#### Recommended fix
Restore multiply-then-divide. The `u128` intermediate is wide enough since `u64::MAX × u64::MAX < u128::MAX`:
```rust
let out = (sol_in as u128)
    .checked_mul(supply as u128)
    .ok_or(StakeError::MathOverflow)?
    .checked_div(total_sol as u128)
    .ok_or(StakeError::MathOverflow)?;
```

### [F-05] `fee_recipient_2` constraint targets wrong pool index (V7)
`unstake.rs:33` binds `fee_recipient_2` to `pool.fee_recipients[1].pubkey` while the `/// CHECK:` comment above it documents `[2]`. The one-character off-by-one means callers must pass slot-1's address as `fee_recipient_2` for the constraint to pass; slot-2's configured pubkey never receives anything.

#### Impact
- An attacker (or admin) who controls `fee_recipients[1]` collects both slot-1 and slot-2 fees, effectively doubling their take when all three slots are populated.
- The legitimate slot-2 recipient never receives the configured share.
- The bug is dormant when only one or two slots are active (unused slots default to zero bps), so it only surfaces when a third fee partner is onboarded — exactly the moment a fee mis-routing is hardest to notice.
- Because `set_fee_recipients` permits two active slots to share a pubkey (see F-19) and the default-initialized state has all three slots set to `Pubkey::default()`, the default state additionally triggers `ConstraintDuplicateMutableAccount` (Custom 2005) in Anchor v2 on every `unstake` until the admin actively configures distinct recipients.

#### Recommended fix
```rust
#[account(mut, address = pool.fee_recipients[2].pubkey @ StakeError::FeeRecipientMismatch)]
pub fee_recipient_2: UncheckedAccount,
```

### [F-06] Missing `token::authority = user` on `user_franksol_ata` in `stake` (V4)
The `user_franksol_ata` account in `stake.rs:30-34` carries only `token::mint = franksol_mint`. The `token::authority = user` constraint that would tie the ATA to the calling user has been removed, so Anchor validates the mint but not who owns the token account.

#### Attack vector
1. Victim calls `stake(amount)`. Attacker constructs (or front-runs with) a transaction where `user_franksol_ata` is the **attacker's own** frankSOL ATA — valid mint, wrong authority.
2. The lamport transfer goes from the victim's wallet to the vault (correct).
3. The victim's `user_position` is initialized with `owner = victim.address()` and `franksol_balance` is incremented (correct).
4. frankSOL is minted to the **attacker's** ATA instead of the victim's.
5. The victim now has a `UserPosition` showing a positive `frankSOL` balance but holds zero frankSOL tokens. Their `unstake` attempt fails when SPL `burn` is invoked against their empty ATA — the staked SOL is permanently stranded.
6. A related attack: the attacker can route freshly minted frankSOL to a non-blacklisted ATA before being blacklisted, defeating the freeze mechanism.

#### Recommended fix
```rust
#[account(
    mut,
    token::mint = franksol_mint,
    token::authority = user,
)]
pub user_franksol_ata: Account<TokenAccount>,
```

### [F-07] Slippage guard checks gross `sol_out` instead of net `user_sol_out` (V2)
At `unstake.rs:86` the slippage check compares the gross redemption amount `sol_out` against `min_sol_out`, but the user actually receives `user_sol_out = sol_out − total_fee`. At the maximum 500 bps configurable fee, the gap is up to 5% — silently bypassing the user's stated minimum.

#### Example
- User unstakes frankSOL worth 1,000 SOL gross, sets `min_sol_out = 960`.
- `sol_out = 1,000 ≥ 960`, check passes.
- `total_fee = 50` (5% cap), `user_sol_out = 950 < 960`. User receives 10 SOL less than they consented to.

An admin who maximizes fees can systematically violate every user's slippage guard by 5%.

#### Recommended fix
```rust
require!(user_sol_out >= min_sol_out, StakeError::SlippageExceeded);
```

### [F-08] `StakeEvent` emitted with stale pool state before update (V6)
The `emit!(StakeEvent { ... })` in `stake.rs:94-100` fires **before** `pool.total_sol` and `pool.franksol_supply` are updated with the new deposit. Off-chain consumers (indexers, dashboards, TVL trackers, automated keepers) therefore see pool totals that are one deposit behind on every event.

#### Impact
On-chain state is correct; the vulnerability lives entirely in the event layer. Consequences:
- Keepers deciding when to call `deploy_to_yield` based on event-derived TVL consistently underestimate the deployable amount.
- Share-price monitoring tools watching `pool_total_sol / franksol_supply` from events compute a stale ratio, masking real-time price moves and arbitrage windows.
- Reorg-replay logic that reconstructs state from events drifts further from reality with every event consumed.

#### Recommended fix
Move the `emit!` to after both state updates:
```rust
pool.total_sol      = PodU64::from(checked_add_u64(pool.total_sol.get(), amount_sol)?);
pool.franksol_supply = PodU64::from(checked_add_u64(pool.franksol_supply.get(), franksol_out)?);
emit!(crate::StakeEvent {
    user: *ctx.accounts.user.address(),
    sol_deposited: amount_sol,
    franksol_minted: franksol_out,
    pool_total_sol: pool.total_sol.get(),
    franksol_supply: pool.franksol_supply.get(),
});
```

### [F-09] `source_vault` passed as readonly_signer in `deploy_to_yield` CPI metas
`yield_generator/src/lib.rs:70` emits `InstructionAccount::readonly_signer(self.source_vault.address())` in the hand-written CPI account list for the inner `system_program::transfer`. The System Program's `Transfer` instruction requires `from` to be **writable** (and a signer, which the vault PDA provides via its signer seeds). The current meta makes the vault read-only, so the inner transfer fails with `PrivilegeEscalation` before any SOL moves.

Three independent layers all need to agree: the Anchor account struct (writable `mut`), the CPI account list constructed in `lib.rs`, and the inner instruction's account flags in `deploy_to_yield.rs::cpi_handle`. The current divergence bricks the entire yield-deployment flow. Distinct from F-01 (which is an access-control bypass): even with F-01 fixed, this CPI privilege bug prevents `deploy_to_yield` from ever succeeding.

#### Recommended fix
```rust
InstructionAccount::writable_signer(self.source_vault.address())
```
Apply consistently across all three layers and add an integration test that actually executes the CPI rather than mocking the inner program.

### [F-10] Cross-program `Account<T>` deserialization fails owner check in `withdraw_from_yield`
`withdraw_from_yield` declares `yield_state` and `user_position` as `Account<YieldState>` and `Account<UserPosition>` — Anchor's typed `Account<T>` deserializer enforces `account.owner == current_program_id`. Both accounts are owned by `yield_generator`, not by `stake_v2`, so deserialization aborts with `IllegalOwner` before the handler ever runs. The entire withdraw-from-yield flow is bricked at the account-validation stage; no CPI fires and no state mutates.

#### Recommended fix
Use `UncheckedAccount` with explicit `seeds::program = yield_generator_program.address()` validation, and (if the data needs to be read) deserialize manually:
```rust
/// CHECK: Owned by yield_generator; validated by seeds.
#[account(seeds = [b"yield_state"], bump, seeds::program = yield_generator_program.address())]
pub yield_state: UncheckedAccount,
```
The same fix applies to `user_position`. If `stake_v2` needs to read `principal` from `YieldState`, deserialize from `yield_state.try_borrow_data()` after verifying the owner manually against `yield_generator::id()`.

### [F-11] Vault PDA prefunding bricks `initialize` for both programs
Both `yield_generator::initialize` and `stake_v2::initialize` guard a System Program `CreateAccount` CPI on the vault PDA using `data_len() == 0`. They never check `lamports`. Because anyone can transfer lamports to a deterministic PDA address before initialization, an attacker can prefund the vault for **0.00089 SOL** (one lamport above the rent threshold), making `CreateAccount` fail with `AccountAlreadyInUse` and bricking deployment of either program.

#### Recommended fix
Either check lamports explicitly:
```rust
require!(vault.lamports() == 0, ErrorCode::VaultAlreadyFunded);
```
or switch to the `Allocate` + `Assign` pattern, which works regardless of pre-existing lamports:
```rust
let cpi_ctx = CpiContext::new_with_signer(system_program, Allocate { account_to_allocate }, signer_seeds);
system_program::allocate(cpi_ctx, 0)?;
let cpi_ctx = CpiContext::new_with_signer(system_program, Assign { account_to_assign }, signer_seeds);
system_program::assign(cpi_ctx, &crate::ID)?;
```

### [F-12] Permissionless `yield_generator::initialize` lets first caller capture `YieldState.authority`
`yield_generator::initialize` accepts any signer as `payer` and writes `state.authority = payer`. There is no `address =` constraint, no upgrade-authority binding, no hardcoded admin. The first caller after deploy permanently captures `set_yield_direction` control.

The blast radius is bounded — the captured authority only controls the yield-direction toggle, not principal — but a hostile authority can flip direction to negative immediately before every withdrawal, applying the reward formula as a loss to drain operator principals over time.

#### Recommended fix
Bind initialization to a known principal. Either pin the program upgrade authority:
```rust
#[account(constraint = upgrade_authority.address() == BPFLoaderUpgradeable_get_authority(&program_data)?)]
pub upgrade_authority: Signer,
```
or hardcode the expected admin pubkey:
```rust
const EXPECTED_ADMIN: Pubkey = pubkey!("...");
#[account(address = EXPECTED_ADMIN @ ErrorCode::Unauthorized)]
pub payer: Signer,
```

### [F-13] Permissionless `stake_v2::initialize` lets first caller capture `Pool.admin`
Same root cause as F-12, applied to `stake_v2::initialize`. The first caller becomes `Pool.admin`, gaining `set_admin`, `set_fund_manager`, `set_fee_recipients`, and `set_user_blacklist`. Higher blast radius than F-12 because the captured role can extract fees (by reassigning fee recipients to attacker-controlled wallets) and arbitrarily freeze/thaw user frankSOL ATAs via the blacklist.

The two issues are distinct from F-11: F-11 bricks initialization by prefunding the vault, F-13 captures admin if initialization succeeds. Both belong to the same "deploy-and-init must be atomic" failure mode.

#### Recommended fix
Same shape as F-12. If both programs share an admin, they should bind to the same pubkey; if not, document and enforce the separation.

### [F-14] `yield_generator::withdraw` accepts arbitrary `destination_vault`
`yield_generator/src/instructions/withdraw.rs:23-25` declares `destination_vault` as an unconstrained `mut UncheckedAccount`. The handler transfers `principal + reward` (or `principal − reward` in loss mode) into whatever account is passed.

Combined with the fund-manager-as-operator model, this means a legitimate fund_manager can sidestep `stake_v2::withdraw_from_yield` entirely: call `yield_generator::withdraw` directly with `destination_vault = attacker_wallet`, drain the deployed SOL plus accrued reward to their own wallet, and `pool.deployed_sol` in `stake_v2` is never decremented. The pool's accounting is permanently desynced from on-chain reality.

Partially blocked today by F-09 (CPI deposit fails), but the design gap stands independent of that. The fix is required even after F-09 is patched.

#### Recommended fix
Constrain `destination_vault` to a canonical address. Either require it to be the `stake_v2` vault PDA:
```rust
#[account(mut, address = derive_stake_vault(stake_program_id))]
pub destination_vault: UncheckedAccount,
```
or require it to sign the withdrawal alongside the operator (forcing the destination owner to consent to receiving funds).

### [F-15] `yield_generator` pays APY from shared vault without solvency check
`yield_generator` holds all operator deposits in a single `yield_vault` PDA. On `withdraw`, the handler checks `yield_vault.lamports() >= principal + reward` but does not check that the **remaining** lamports would still cover all other open positions. Rewards are therefore paid out of other operators' principal, leaving the vault undercollateralized.

#### Example
- Operator A deposits 100 SOL, operator B deposits 100 SOL. Vault holds 200 SOL.
- After 1 year, A withdraws. APY = 10%. Withdrawal pays 110 SOL.
- Vault remaining: 90 SOL.
- B's position still shows 100 SOL principal. Their withdrawal can no longer be fully paid; first-out wins.

#### Recommended fix
Track aggregate `total_principal` on `YieldState` and enforce solvency on withdrawal:
```rust
let vault_after = vault.lamports().checked_sub(total_out)?;
let remaining_principal = state.total_principal.checked_sub(principal_returned)?;
require!(vault_after >= remaining_principal, ErrorCode::VaultUndercollateralized);
```

### [F-16] `operator` marked readonly_signer in `withdraw_from_yield` CPI metas
Same class of bug as F-09, on the other side of the deploy/withdraw pair. The manual CPI meta list for `yield_generator::withdraw` marks `operator` as readonly_signer when the callee expects writable_signer (it pays for `UserPosition` close-rent return and is used as a writable account by the inner SOL transfer accounting). The result is `PrivilegeEscalation` and a failing CPI before any state moves.

#### Recommended fix
Mark `operator` as `writable_signer` in the CPI account list, and add an integration test that actually executes the CPI end-to-end rather than asserting on mocked sub-calls.

### [F-17] `set_yield_direction` doesn't update `last_config_update_ts`
`set_yield_direction` updates `yield_direction_positive` but never writes `state.last_config_update_ts`. The field is documented in `PROGRAM_GUIDE.md:82-89` as tracking the most recent configuration change, but it stays frozen at initialization time after any direction flip. Low severity: no funds are at risk, but any off-chain monitoring or governance tooling reading this field for "config has changed recently" signals is silently misled.

#### Recommended fix
```rust
state.last_config_update_ts = PodI64::from(Clock::get()?.unix_timestamp);
```

### [F-18] `set_admin` accepts `Pubkey::default()` and permanently locks admin controls
`set_admin` has no guard against `new_admin == Pubkey::default()`. An authorized admin misconfiguration (paste error, scripting bug) sets `pool.admin = 11111111111111111111111111111111` (the System Program's address). Since no keypair signs as the System Program, `set_admin`, `set_fund_manager`, `set_fee_recipients`, and `set_user_blacklist` become permanently uncallable.

Same gap exists in `set_fund_manager`.

#### Recommended fix
```rust
require_keys_neq!(new_admin, Pubkey::default(), StakeError::InvalidAdmin);
```
Apply the same guard to `set_fund_manager` and any other role-rotation handler.

### [F-19] `set_fee_recipients` permits duplicate active recipients, bricking `unstake`
`set_fee_recipients` validates the aggregate bps cap but not per-slot uniqueness. If two active slots share the same pubkey, `unstake` must list that pubkey twice in the writable account positions, which Anchor v2 rejects with `ConstraintDuplicateMutableAccount` (Custom 2005). The DoS persists until the admin calls `set_fee_recipients` again with unique pubkeys.

Same root admits a second variant: a slot configured with `pubkey = Pubkey::default()` and nonzero bps silently routes fees to the System Program (where they are unrecoverable).

#### Recommended fix
Validate per-slot integrity inside `set_fee_recipients`:
```rust
let active: Vec<_> = recipients.iter().filter(|r| r.active).collect();
let pubkeys: HashSet<_> = active.iter().map(|r| r.pubkey).collect();
require!(pubkeys.len() == active.len(), StakeError::DuplicateFeeRecipient);
for r in &active {
    require_keys_neq!(r.pubkey, Pubkey::default(), StakeError::InvalidFeeRecipient);
    require!(r.bps > 0 || !r.active, StakeError::InvalidFeeConfig);
}
```

### [F-20] `frankSOL` holders without a `UserPosition` cannot `unstake`
The `Unstake` context loads `user_position` via `seeds = [b"user_position", user.address()]` without `init_if_needed`, so any address that holds `frankSOL` purely from an SPL transfer (never having called `stake`) fails account validation on `unstake`.

The lock is recoverable in practice — the holder can call `stake` with any non-zero amount to `init` the PDA, then `unstake` their full balance (`unstake` uses `saturating_sub` on the mirror balance). But this contradicts the LST model the protocol presents: `frankSOL` is a transferable SPL token whose holders should not need to deposit first to redeem.

#### Recommended fix
Use `init_if_needed` on `user_position` in the `Unstake` context, or expose a separate one-shot `claim_position` instruction that just creates the PDA for secondary-market holders. Be explicit in user-facing docs about which path applies.

### [F-21] `set_fund_manager` allows rotation while `deployed_sol > 0`, deadlocking `withdraw_from_yield`
The `yield_generator` position is seeded by `[b"user_position", fund_manager]`. If `set_fund_manager` rotates the fund manager while `pool.deployed_sol > 0`, the on-chain `UserPosition` PDA in `yield_generator` is still seeded by the **old** fund_manager's pubkey:

- The new fund_manager fails the position-owner check inside `yield_generator::withdraw`.
- The old fund_manager fails the `pool.fund_manager == signer` check inside `stake_v2::withdraw_from_yield`.

Deployed SOL is irrecoverable unless the admin rotates back to the old fund_manager — possible only if the old key is still available. Operational risk, not an external exploit.

#### Recommended fix
Block rotation while SOL is deployed:
```rust
require!(pool.deployed_sol.get() == 0, StakeError::RotationBlocked);
```
Or introduce a position-migration path that re-keys the `yield_generator::UserPosition` PDA under the new fund manager.

### [F-22] `UserPosition` accounts are never closed (rent never recovered)
`stake_v2::unstake` and `yield_generator::withdraw` decrement the position's balance fields but never call `close` on the `UserPosition` PDA when the balance reaches zero. The rent (~0.00089 SOL per position) is permanently locked, and the dust accumulates across the entire user base over time.

#### Recommended fix
After zeroing the position, close it back to the user:
```rust
if user_position.franksol_balance.get() == 0 {
    user_position.close(user.to_account_info())?;
}
```

### [F-23] `Treasury` declared `UncheckedAccount` with no address validation
The `Treasury` (initial fee recipient on `initialize`) is declared as `UncheckedAccount` with no `address =` constraint and no equivalent runtime check. Any account passed as `treasury` becomes the first fee recipient at 500 bps. If the deployer scripts the wrong address, the misconfiguration is silent until the first `unstake` routes fees to the wrong account.

#### Recommended fix
Either pin treasury to a known PDA (`#[account(seeds = [b"treasury"], bump)]`) or to a constant pubkey (`#[account(address = TREASURY_PUBKEY)]`). Document which model is intended.

### [F-24] Permissionless `yield_generator::deposit`
`yield_generator::deposit` accepts any signer as `operator`. Combined with the shared-vault model (F-15), this lets any address open positions in `yield_generator` and earn 10% APY from the same vault that's paying out legitimate operators' principal. Low because the attacker still needs to deposit SOL themselves to get a reward, but the design makes the vault a public lending pool rather than a private strategy contract.

#### Recommended fix
Gate `deposit` on `YieldState.allowed_operators` or on `operator == stake_v2::derive_fund_manager()`. The "deposits only from `stake_v2`" model would also remove the need for F-14's `destination_vault` constraint.

### [F-25] `set_user_blacklist` cannot preemptively blacklist a user without a `UserPosition`
`set_user_blacklist` loads `user_position` without `init_if_needed`, so a known-malicious address that hasn't yet staked cannot be added to the blacklist. The admin must wait until the user actually stakes (initializing the PDA) before they can be frozen — a race condition that defeats the purpose of preemptive blacklisting.

#### Recommended fix
Either `init_if_needed` the position inside `set_user_blacklist` (creating an empty position just to carry the flag), or move the blacklist into a separate Bloom-filter-style PDA keyed by the global pool rather than per-user.

### [F-26] Yield direction change applied retroactively to in-flight positions
When `set_yield_direction` flips `yield_direction_positive`, all in-flight positions are settled at the new direction on their next withdrawal — including the time period during which they were accruing under the old direction. A position deposited under positive yield can be withdrawn at a loss simply because the authority toggled direction shortly before withdrawal.

#### Recommended fix
Snapshot the direction on each position at deposit time:
```rust
pub struct UserPosition {
    // ...
    pub direction_at_entry: bool,
}
```
And use `position.direction_at_entry` rather than `state.yield_direction_positive` when settling withdrawals.

---

*Solana Audit Arena — Week 4. Sources: [WEEK_4.md](../FrankSol/FrankSol/WEEK_4.md), [VULNERABILITIES.md](../VULNERABILITIES.md), [FrankSol_results.md](../FrankSol_results.md).*
