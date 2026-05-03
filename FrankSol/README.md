# stake_v2

A two-program Solana staking system built with [Anchor](https://www.anchor-lang.com/). Users deposit SOL, receive a liquid receipt token (`frankSOL`), and the protocol’s fund manager routes that capital into a pluggable yield strategy. The strategy simulates real-world market conditions through a configurable yield direction (profit **or** loss). **Protocol fees are collected on every unstake** and split across up to three configurable fee recipients; the **sum of all recipient fee basis points** is capped at **5% (500 bps)**.

---

## Migration status

This repository has been migrated to **Anchor v2** (`anchor-next` branch ecosystem), including:

- `anchor-lang-v2` / `anchor-spl-v2`,
- Pod zero-copy state layouts for program accounts,
- v2 handler/context/CPI conventions,
- updated build/test harnesses.

For detailed technical documentation:

- Full migration notes (with before/after code snippets): `ANCHOR_V2_MIGRATION_NOTES.md`
- Setup/revert quick guide: `ANCHOR_V2_SETUP_QUICKSTART.md`
- `stake_v2` program developer guide: `programs/stake_v2/PROGRAM_GUIDE.md`
- `yield_generator` program developer guide: `programs/yield_generator/PROGRAM_GUIDE.md`

---

## Repository layout

```
stake_v2/
├── programs/
│   ├── stake_v2/          # Core staking pool (users interact here)
│   │   └── src/
│   │       ├── lib.rs              # Program entry-points
│   │       ├── state.rs            # Pool, UserPosition, FeeRecipient, FeeRecipientArg
│   │       ├── constants.rs        # Seeds, MAX_TOTAL_FEE_BPS, precision
│   │       ├── error.rs            # Error enum
│   │       ├── utils.rs            # Math helpers & CPI wrappers
│   │       └── instructions/
│   │           ├── initialize.rs   # One-time pool + vault creation
│   │           ├── stake.rs        # Deposit SOL → mint frankSOL
│   │           ├── unstake.rs      # Burn frankSOL → receive SOL (multi-recipient fee)
│   │           ├── deploy_to_yield.rs     # Send liquid SOL to yield_generator
│   │           ├── withdraw_from_yield.rs # Pull SOL back + settle PnL
│   │           └── admin.rs        # Admin-only config instructions
│   │
│   └── yield_generator/   # Isolated yield strategy program
│       └── src/
│           ├── lib.rs                # Program entry + manual CPI surface
│           ├── constants.rs          # STATE_SEED, VAULT_SEED, APY_BPS, etc.
│           ├── state.rs              # YieldState, UserPosition (Pod layout)
│           ├── error.rs              # Error enum
│           └── instructions/         # initialize/deposit/withdraw/set_yield_direction
│
├── Anchor.toml
└── Cargo.toml
```

---

## Programs

### 1. `stake_v2` — Staking Pool

**Program ID:** `tsXqrLjaTSNeyCEMTBAHPZkyDvxRHBuv1t29f3rkZpz`

The pool is the primary interface for users and the fund manager. It maintains two main PDAs (plus a `UserPosition` PDA per staker for accounting and policy):

| PDA | Seeds | Purpose |
|-----|-------|---------|
| `Pool` | `[b"pool"]` | Global state: balances, roles, fee configuration |
| Vault | `[b"vault"]` | System-owned SOL custody account |
| `UserPosition` (per user) | `[b"user_position", user_pubkey]` | Mirrored `frankSOL` balance, cost basis, blacklist flag |

#### Pool state

```
FeeRecipient {
    pubkey:  Pubkey,   // destination for that slot; Pubkey::default() + 0 bps = inactive
    fee_bps: u16,      // fee for this slot, in basis points
}

Pool {
    admin:            Pubkey,                  // can rotate roles and fee config
    fund_manager:     Pubkey,                  // can deploy/withdraw capital
    fee_recipients:   [FeeRecipient; 3],       // up to 3 fee slots; total fee_bps ≤ 500
    franksol_mint:    Pubkey,                  // frankSOL SPL-token mint
    total_sol:        u64,                     // total SOL tracked by the pool
    deployed_sol:     u64,                     // SOL currently deployed to yield_generator
    franksol_supply:  u64,                     // circulating frankSOL supply mirror
    bump:             u8,
    vault_bump:       u8,
    mint_auth_bump:   u8,
}
```

#### `UserPosition` (stake_v2) — staker account

Each user with an open position has a PDA for mirrored balance, accounting, and policy:

```
UserPosition {
    owner:            Pubkey,
    franksol_balance: u64,      // mirror of ATA (updated on program stake/unstake)
    sol_deposited:    u64,      // cost basis; does not decrease on unstake
    is_blacklisted:   bool,     // if true, stake/unstake are blocked
    bump:             u8,
}
```

#### Instructions

| Instruction | Signer | Description |
|-------------|--------|-------------|
| `initialize` | admin | Creates the `Pool`, vault PDA, and `frankSOL` mint. Sets `fund_manager` and **initial fee slot 0** from the `treasury` account (pubkey) at **`MAX_TOTAL_FEE_BPS` (5%)**; other slots start empty. |
| `stake` | user | Transfers SOL from the user to the vault. Mints proportional `frankSOL` (slippage-protected via `min_franksol_out`). |
| `unstake` | user | Burns `frankSOL`. Splits `sol_out` against each slot’s `fee_bps` (capped in aggregate by `set_fee_recipients`); user receives `sol_out − total_fees`. Requires three accounts `fee_recipient_0/1/2` whose **keys must match** `pool.fee_recipients[i].pubkey` (enforced in the program). Slippage via `min_sol_out` (pre-fee gross). |
| `deploy_to_yield` | fund_manager | CPIs into `yield_generator::deposit` to move liquid SOL into the yield strategy. Increases `deployed_sol`. |
| `withdraw_from_yield` | fund_manager | CPIs into `yield_generator::withdraw`. Observes actual vault delta, settles positive or negative PnL into `total_sol`. |
| `set_admin` | admin | Rotates the admin key. |
| `set_fund_manager` | admin | Rotates the fund manager key. |
| `set_fee_recipients` | admin | Replaces all three `FeeRecipient` slots. **Sum of `fee_bps` must be ≤ 500** (`MAX_TOTAL_FEE_BPS`). |
| `set_user_blacklist` | admin | Toggles `UserPosition.is_blacklisted` for a user (stake/unstake blocked when true). |

---

### 2. `yield_generator` — Yield Strategy

**Program ID:** `EKHVBmv9LcrRC64ryycKWphdkVdM1k2sVETbBvgSxCrf`

A self-contained strategy program that accepts SOL deposits from an operator (the `stake_v2` vault), accrues time-weighted rewards at a fixed APY, and returns principal ± accrued yield on withdrawal. Yield direction (gain vs. loss) is runtime-configurable to simulate real market conditions.

| PDA | Seeds | Purpose |
|-----|-------|---------|
| `YieldState` | `[b"yield_state"]` | Global strategy config and totals |
| Yield vault | `[b"yield_vault"]` | SOL custody account owned by this program |
| `UserPosition` | `[b"user_position", operator_pubkey]` | Per-operator deposit record |

#### YieldState

```
YieldState {
    authority:                Pubkey,  // set at init; only they can call set_yield_direction
    apy_bps:                  u16,     // 1 000 = 10% APY (hardcoded at init)
    total_principal:          u64,
    total_yield_paid:         u64,
    last_config_update_ts:    i64,
    state_bump:               u8,
    vault_bump:               u8,
    yield_direction_positive: bool,    // true = gain, false = loss
}
```

#### UserPosition (yield_generator)

```
UserPosition {
    owner:           Pubkey,  // operator who made the deposit
    principal:       u64,
    accrued_reward:  u64,     // accumulated since last_update_ts
    last_update_ts:  i64,
    bump:            u8,
}
```

#### Instructions

| Instruction | Signer | Description |
|-------------|--------|-------------|
| `initialize` | payer | Creates `YieldState` and the yield vault PDA. Sets payer as authority. Defaults `yield_direction_positive = true`. |
| `deposit` | operator | Creates a `UserPosition`, transfers SOL from `source_vault` (must co-sign) to the yield vault. One position per operator. |
| `withdraw` | operator | Accrues time-weighted rewards, then transfers `principal ± reward` back to the destination vault and closes the position. |
| `set_yield_direction` | authority | Flips `yield_direction_positive`. Authority must match `YieldState.authority`. |

---

## Key features

### frankSOL — liquid receipt token

`frankSOL` is an SPL token (9 decimals) minted 1:1 with deposited SOL at genesis, then tracked using a proportional price formula:

```
frankSOL_out = sol_in × franksol_supply / total_sol
sol_out      = franksol_in × total_sol / franksol_supply
```

As `total_sol` grows (positive yield) or shrinks (negative yield), each `frankSOL` token represents a proportional share of the pool — identical in concept to liquid staking tokens (stSOL, mSOL, etc.).

---

### Yield direction — positive and negative

The `yield_generator` authority can toggle yield direction at any time:

```
set_yield_direction(is_positive: true)   // normal: user gets principal + accrued_reward
set_yield_direction(is_positive: false)  // bear:   user gets principal - accrued_reward (min 0)
```

**Reward accrual formula (both directions):**

```
reward = principal × apy_bps × elapsed_seconds
         ────────────────────────────────────────
              BPS_DENOMINATOR × SECONDS_PER_YEAR
```

`BPS_DENOMINATOR = 10 000`, `SECONDS_PER_YEAR = 31 536 000`, `APY = 10%`.

When direction is negative:
- The withdrawal uses `saturating_sub` so the vault never pays more than it holds.
- `total_yield_paid` is **not** incremented (losses are not revenue).
- `stake_v2` settles the loss: `pool.total_sol` decreases, which automatically deflates the `frankSOL` price.

---

### Protocol fees — on unstake (up to three recipients, 5% cap)

Fees are **not** a single hardcoded treasury; they are stored on-chain in `Pool.fee_recipients[0..3]`. For each non-zero `fee_bps` slot, the program computes a fee on the **gross** `sol_out` from the burn, then sends that lamport amount from the vault to the corresponding `pubkey`.

```
fee_i        = sol_out × fee_bps_i / BPS_DENOMINATOR
total_fee    = sum(fee_i) over active slots
user_receives = sol_out - total_fee
```

**Constraints:**
- **Admin** sets all three slots in one call via `set_fee_recipients`. The **sum of all `fee_bps`** must not exceed `MAX_TOTAL_FEE_BPS` (500 = 5%).
- **`unstake` accounts** must include `fee_recipient_0`, `fee_recipient_1`, and `fee_recipient_2` with keys **equal** to `pool.fee_recipients[0/1/2].pubkey` (program-enforced; wrong keys fail validation). The client should **fetch the pool** and pass those three pubkeys in order.
- Slots with `fee_bps == 0` do not move lamports; their pubkey is still part of the account list and must still match the stored value (including `Pubkey::default()` for empty slots, if that is what is stored).

`initialize` still accepts a `treasury` account for the **first slot** only and seeds it with **`MAX_TOTAL_FEE_BPS` on that slot** so a fresh pool behaves like the old single-5%-treasury design until the admin reconfigures via `set_fee_recipients`.

---

### PnL settlement in `withdraw_from_yield`

After the CPI completes, `stake_v2` computes the actual vault delta rather than trusting a caller-supplied number:

```
vault_delta = vault_after - vault_before          // actual SOL returned
pnl         = vault_delta - principal_returned    // signed profit/loss

if pnl >= 0:  pool.total_sol += pnl   // yield passed through to stakers
else:         pool.total_sol -= |pnl| // loss socialised across stakers
```

This makes `frankSOL` pricing automatically reflect real yield outcomes.

---

## Architecture diagram

```
  User
   │  stake(SOL)          unstake(frankSOL)
   │──────────────────────────────────────▶  stake_v2
   │                                              │
   │  ◀── mint frankSOL        burn frankSOL ──▶  │
   │  ◀── SOL (net of fees)   fees ──▶ fee_recipient_0/1/2 (per pool config)
   │                                              │
   │                          deploy_to_yield ───▶│──CPI──▶ yield_generator::deposit
   │                       withdraw_from_yield ──▶│──CPI──▶ yield_generator::withdraw
   │                                              │
   │                                         admin│
   │         set_admin / set_fund_manager / set_fee_recipients / set_user_blacklist
   │
   │                                   yield_generator authority
   │                                         set_yield_direction(true/false)
```

---

## Building and testing

**Prerequisites:** Rust stable, Solana CLI, Anchor v2 (`anchor-next`), Yarn.

Install Anchor v2:

```bash
cargo install --git https://github.com/solana-foundation/anchor.git --branch anchor-next anchor-cli --force
```

macOS fallback if linker/bitcode errors occur:

```bash
CARGO_PROFILE_RELEASE_LTO=off cargo install --git https://github.com/solana-foundation/anchor.git --branch anchor-next anchor-cli --force
```

For rollback to stable Anchor v1 and AVM-based switching, see `ANCHOR_V2_SETUP_QUICKSTART.md`.

```bash
# Build and test full workspace
cargo check --manifest-path Cargo.toml
cargo test --manifest-path Cargo.toml

# Optional Anchor CLI flow
anchor build
anchor test

# Focused crate test runs
cargo test --manifest-path programs/stake_v2/Cargo.toml
cargo test --manifest-path programs/yield_generator/Cargo.toml
```

---

## Constants reference

### `stake_v2` — `src/constants.rs`

| Constant | Value | Purpose |
|----------|-------|---------|
| `POOL_SEED` | `b"pool"` | Seed for Pool PDA |
| `VAULT_SEED` | `b"vault"` | Seed for SOL vault PDA |
| `MINT_AUTH_SEED` | `b"mint_auth"` | Seed for mint/freeze authority PDA |
| `FRANKSOL_MINT_SEED` | `b"franksol_mint"` | Seed for frankSOL SPL mint |
| `USER_POSITION_SEED` | `b"user_position"` | Seed prefix for per-user position |
| `MAX_TOTAL_FEE_BPS` | `500` | Maximum total fee across all slots (5%) |
| `BPS_DENOMINATOR` | `10 000` | Denominator for BPS calculations |
| `PRECISION` | `1_000_000_000` | Fixed-point precision multiplier |

### `yield_generator` — `src/constants.rs`

> These constants are re-exported from `yield_generator::*` so callers can write
> `yield_generator::STATE_SEED` instead of hard-coding string literals.

| Constant | Value | Purpose |
|----------|-------|---------|
| `STATE_SEED` | `b"yield_state"` | Seed for YieldState PDA |
| `VAULT_SEED` | `b"yield_vault"` | Seed for yield vault PDA |
| `POSITION_SEED` | `b"user_position"` | Seed prefix for per-operator position |
| `APY_BPS` | `1 000` | Default APY (10%) written at init |
| `BPS_DENOMINATOR` | `10 000` | Denominator for BPS calculations |
| `SECONDS_PER_YEAR` | `31 536 000` | Used in the reward accrual formula |

### Errors (stake_v2) — selected

| Variant | When |
|--------|------|
| `FeesExceedCap` | `set_fee_recipients` would set total `fee_bps` above `MAX_TOTAL_FEE_BPS` |
| `FeeRecipientMismatch` | An `unstake` fee recipient account key does not match `pool.fee_recipients[i].pubkey` |
| `UserBlacklisted` | User is blacklisted for stake/unstake |
