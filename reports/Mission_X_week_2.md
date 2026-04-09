# Security Audit: MissionX Protocol
24.03.26 - 29.03.26

## Acknowledgements
The report is written by [die-kreatur](https://github.com/die-kreatur). It was published before Frank Castle's official judging and represents an independent analysis.
Since all findings were public, the author reviewed submissions from other participants and included the most interesting ones from [novoyd](https://github.com/novoyd).

This report may be reproduced or referenced with attribution to the original author.

## Overview
_Framework: Anchor 0.30.1 · Token-2022 compatible_

The audited protocol is MissionX, which is an on-chain bounty marketplace inspired by the pump.fun model.

This audit was performed as part of [Solana Audit Arena - Week 2](https://github.com/Frankcastleauditor/Solana-Audit-Arena), a weekly security competition organized by Frank Castle.

## Scope
All code in `programs/missionx/src/`

## Architecture Summary

### Protocol roles and management
- There are five main roles: protocol owner, moderator (can be multiple), mission creator, player (up to three per mission), executor to migrate a mission.
- Protocol parameters are stored in Configuration PDA, which is created during the initialization by the owner.
- Config can be changed by the protocol owner, but it doesn't affect existing missions.
- Moderators can approve or censor a new mission, block an active mission and optionally ban its trading, unblock a mission and restore its previous trade status, permanently fail a blocked mission.

### General Flow
- Anyone can create a mission by depositing a SOL payout into a `Missionx` PDA and minting a bonding-curve token tied to that mission. Each mission inherits some parameters from the global config. Only one mission per token mint can be created.
- After a mission is created, a moderator can either approve or censor it.
- An open mission can be accepted by any player. Up to three players can participate in a mission.
- When a mission is accepted, it becomes open for trading, which means anyone can buy and sell tokens.
- If no one accepted a mission, it can be labeled as failed.
- A moderator or the creator can complete the mission, marking it as successful or not. If the mission is not successful, it is opened for players again.
- If the migration threshold is reached, no one can buy tokens any more and a mission is meant to be migrated.
- The protocol also supports a grace period system for failed missions, allowing token holders to sell before funds are fully locked.

### Two Parallel Lifecycles
                                                                                                                                                                                                        
Each mission has its own lifecycle and a parallel trading lifecycle. They are governed by two independent variables:
                                                                                                                                                                                                        
```             
missionx_status:  Unverified → Open → Accepted → Completed
                                    ↘ Failed → Withdrawn
                                                                                                                                                                                                
trade_status:     Closed → Open → MigrationRequired → Migrated
                                ↘ Banned                                                                                                                                                                
```             

`missionx_status` tracks the task lifecycle. `trade_status` tracks the token trading lifecycle. Transitions between the two are not always synchronized.

### Bonding Curve
There are two formulas:
- `buy_amount * full_sol_reserve / (full_token_reserve - buy_amount) + 1`
- `sell_amount * full_sol_reserve / (sell_amount + full_token_reserve)`

Formulas rely on virtual reserves `v0` (SOL) and `v1` (tokens). Real reserves are `reserve0` and `reserve1`. All calculations operate on `full_reserve = virtual + real`.                                                                                                                             
When `reserve0` reaches `migration_threshold`, `trade_status` transitions to `MigrationRequired` and the executor claims the liquidity.

## Design concerns
- Safe math is not enforced everywhere. This can lead to a division by zero panic in `buy.rs` when `buy_amount == full_token_reserve`, and to reverted transactions with no clear error message due to integer overflow.
- `ban_active` and `unban_active` use a blacklist approach, excluding only `Unverified` status instead of an explicit whitelist of allowed statuses. This creates state machine inconsistencies where moderation flows can be applied to terminal states such as `Completed`, `Failed`, `Migrated`, and `Withdrawn`. Several concrete exploit paths are documented in this report, but the blacklist pattern may allow additional undiscovered vectors.

## Findings Summary
| ID | Severity | Title |
|----|---------|-------|
| F-01 | High | `switch_ban_to_failed` with `immediate=true` is broken |
| F-02 | High | Anyone can buy full supply including reserved tokens |
| F-03 | Medium | `Completed` mission may be marked as `Failed` and `Withdrawn` |
| F-04 | Medium | `Migrated` mission may be reopened |
| F-05 | Medium | `MigrationRequired` trade status may be bypassed |
| F-06 | Medium | `ConfirmAccounts` and `MigrateAccounts` exceed BPF stack limit |
| F-07 | Medium | Slippage protection is checked against gross amount in both `buy` and `sell` |
| F-08 | Medium | Setting `metadata_authority` causes a protocol-wide DoS on mission creation |
| F-09 | Low | A single player can occupy all three submitter slots |
| F-10 | Low | Unbounded `v0`/`v1` parameters may corrupt AMM pricing |
| F-11 | Low | Fee parameters are unbounded in `init` and `set_global_config` |

## Detailed Findings
### [F-01] `switch_ban_to_failed` with `immediate=true` is broken

The `immediate` flag is intended to start the grace period at the current time. Instead, the implementation sets `fail_ts = Some(0)`:

```rust
missionx_state.fail_ts = if immediate { Some(0) } else { Some(clock.unix_timestamp as u64) };
```

`fail_ts` is used as a Unix timestamp in two places: `ensure_missionx_tradable` allows trading only while `current_time <= fail_ts + fail_grace_period` (until the end of the grace period), and `withdraw_from_missionx` allows the owner to withdraw once `fail_ts + fail_grace_period < current_time`.

With `fail_ts = 0`, both conditions evaluate as if the mission failed at 01.01.1970. Since the grace period expired decades ago, calling `switch_ban_to_failed(immediate=true)` has two immediate consequences:
- token holders can no longer call `sell`;
- the owner can call `withdraw_from_missionx` without waiting for the grace period.

#### Recommended Fix

Always set `fail_ts` to the current timestamp regardless of the `immediate` flag. If the intent is to allow withdrawal without delay, the `immediate` path should be removed or handled differently, not by using a fake timestamp.

```rust
missionx_state.fail_ts = Some(clock.unix_timestamp as u64);
```

### [F-02] Anyone can buy full supply including reserved tokens
`buy_amount` is never validated in the `buy` instruction. When trading starts, `reserve1` records only 98.5% of the total token supply, while the entire supply is held by the token vault. Thus, an attacker can buy the full supply, which leads to:
- `reserve1` underflow: `missionx_state.reserve1 -= effective_buy_amount`;
- reserved tokens are gone and cannot be paid out.

#### Recommended Fix
Add validation that `buy_amount` does not exceed the tradeable supply or any other order amount:
```rust
require!(buy_amount < missionx_state.reserve1, MissionxErrors::OrderTooBig)
```

### [F-03] Completed mission may be marked as failed and withdrawn
`ban_active` filters missions by blacklist status: the only excluded status is `Unverified`. That means that a mission with any other status may be banned. Thus, a successfully completed mission may be passed to `ban_active` and `is_blocked` would be set to `true`. `switch_ban_to_failed(immediate=true)` can then be called and the completed mission will be marked as failed. Since `fail_ts` is not a valid timestamp when `immediate=true` (read F-01), the protocol owner can immediately withdraw all remaining funds. This breaks the documented invariant that `Completed` is a terminal state.

#### Recommended Fix
Use whitelists of allowed statuses in both `ban_active` and `unban_active`. For instance:
```rust
require!(
    matches!(
        missionx_state.missionx_status,
        MissionxStatus::Open | MissionxStatus::Accepted
    ),
    MissionxErrors::WrongMissionxSatus
);
```

### [F-04] `Migrated` mission may be reopened

This finding combines two independent oversights that together enable draining the creator's escrowed payout after migration.

**Stale reserves after migration.** The `migrate` instruction transfers `reserve1` tokens to the executor and `reserve0` SOL minus the migration fee, but never zeroes these fields in state. After migration, `reserve0` and `reserve1` still reflect the pre-migration AMM balances even though the actual funds are gone.

**`Missionx` PDA still holds creator SOL.** The `Missionx` PDA acts as a treasury: it holds both `reserve0` (trading SOL) and `payout_amount` (the creator's deposit). Migration only removes `reserve0`. The creator's deposit remains on the PDA balance.

**Trading can be reopened.** As described in F-03, `ban_active` does not exclude `Migrated` trade status. A moderator can call `ban_active(ban_sell=false)` on a migrated mission, leaving `old_trade_status = None`. `switch_ban_to_failed` then passes its `old_trade_status` check and sets `trade_status = Open`, reopening trading on a mission that should be permanently closed.

**Exploit path.** Once trading is reopened, users can call `sell` which calculates the SOL payout using stale `reserve0` and `reserve1` values. Since the real AMM liquidity is gone, the payment comes from whatever SOL remains on the PDA: the creator's `payout_amount`. Sellers effectively drain the creator's deposit at prices based on phantom reserves.

#### Recommended Fix
Reset `reserve0` and `reserve1` in the `migrate` instruction after transferring liquidity:
```rust
missionx_state.reserve0 = 0;
missionx_state.reserve1 = 0;
```
Additionally, applying the whitelist fix from F-03 to `ban_active` would prevent moderation flows from reopening migrated missions entirely.

### [F-05] `MigrationRequired` trade status may be bypassed

The same root cause as F-03 and F-04 applies here. A moderator can call `ban_active(ban_sell=false)` on a mission in `MigrationRequired` state, which sets `is_blocked = true` but leaves `old_trade_status = None`. `switch_ban_to_failed` then passes its check and resets `trade_status` back to `Open`.

This permanently bypasses migration — `migrate` requires `trade_status == MigrationRequired` and becomes inaccessible. Trading continues past the threshold indefinitely, and the executor can never claim the liquidity.

#### Recommended Fix

Applying the whitelist fix from F-03 to `ban_active` would prevent this. Additionally, `MigrationRequired` should be treated as an irreversible state. `switch_ban_to_failed` should explicitly reject missions where `trade_status` is `MigrationRequired` or `old_trade_status` was `MigrationRequired`.

### [F-06] `ConfirmAccounts` and `MigrateAccounts` exceed BPF stack frame limit

The BPF VM enforces a 4096-byte stack frame limit per instruction. Both `ConfirmAccounts` and `MigrateAccounts` place large account types directly on the stack without boxing them.
In `ConfirmAccounts`, `token_vault_pda`, `creator_ata`, and `player_ata` are unboxed `InterfaceAccount<TokenAccount>` fields. Combined with the rest of the struct, the frame size reaches ~4352 bytes.

In `MigrateAccounts`, five token accounts are unboxed: `token_vault_pda`, `executor_ata`, `creator_ata`, `player_ata`, and `fee_recipient_ata`. The combined frame size reaches ~5568 bytes.

Thus, both `complete_missionx` and `migrate` will panic at runtime due to stack overflow, making them permanently unusable.

#### Recommended Fix
Wrap the unboxed token accounts in `Box` smart pointer to move them to the heap:
```rust
pub token_vault_pda: Box<InterfaceAccount<'info, token_interface::TokenAccount>>,
pub creator_ata: Box<InterfaceAccount<'info, token_interface::TokenAccount>>,
pub player_ata: Box<InterfaceAccount<'info, token_interface::TokenAccount>>,
```

### [F-07] Slippage protection is checked against gross amount in both `buy` and `sell`

Both `buy` and `sell` validate slippage against the gross amount before fees, not against what the user actually sends or receives.
In `sell`, the user specifies `min_out` as the minimum SOL they expect to receive. The slippage check passes if the gross output meets this threshold, but the fee is deducted afterwards. When fees are non-zero, the user receives less SOL than they specified as their minimum.

In `buy`, the user specifies `pay_cap` as the maximum SOL they are willing to spend. The slippage check passes if the gross cost is within this cap, but the fee is added on top. When fees are non-zero, the user pays more than their specified maximum.

In both cases the slippage guard is effectively bypassed when fees are non-zero.

#### Recommended Fix

In `sell`, validate `min_out` against the net amount after fee deduction. In `buy`, validate `pay_cap` against the total spend including fee.

### [F-08] Setting `metadata_authority` causes a protocol-wide DoS on mission creation

In `create_missionx`, the mint account space is calculated with an empty extension list, the account is created at that size, and only then the `MetadataPointer` extension is conditionally pushed to the list and initialized. Token-2022 requires extension space to be allocated at account creation time. Extensions cannot be added to an account that was not sized for them.

As a result, if the owner sets `metadata_authority` to any non-`None` value via `init` or `set_global_config`, every call to `create_missionx` reverts with `InvalidAccountData`. Mission creation is completely blocked until `metadata_authority` is reset to `None`.

#### Recommended Fix

Build the full extension list before calculating account space:
```rust
let mut token_extensions: Vec<ExtensionType> = vec![];
if ctx.accounts.config.metadata_authority.is_some() {
    token_extensions.push(ExtensionType::MetadataPointer);
}
let mint_space = ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(&token_extensions)?;
```

### [F-09] A single player can occupy all three submitter slots

`accept_missionx_multi` does not check whether the caller is already registered in `submitters`. A player who accepted a mission via `accept_missionx` can call `accept_missionx_multi` twice and occupy all three slots, blocking other players from joining.

There is no financial impact: payout is always sent to a single player on `complete_missionx(is_successful=true)`. The severity is Low because the impact is limited to a DoS on other players' participation.

#### Recommended Fix

Add a check in `accept_missionx_multi` that the caller is not already in `submitters`:
```rust
require!(
    !missionx_state.submitters.iter().any(|s| s.is_some_and(|k| k == ctx.accounts.player.key())),
    MissionxErrors::AlreadyJoined
);
```

### [F-10] Unbounded `v0`/`v1` parameters may corrupt AMM pricing

`v0` and `v1` (virtual SOL and token reserves) are set by the owner via `init` and `set_global_config` without any bounds validation. This creates two potential misconfiguration scenarios.

**Excessively large values.** `get_full_sol_reserve` and `get_full_token_reserve` compute `v0 + reserve0` and `v1 + reserve1` using unchecked addition. If the owner sets an excessively large value (e.g. a typo adding an extra digit), the addition silently overflows and corrupts AMM pricing for all missions created after the change. For instance, if admin accidentally sets `v1 = 18_000_000_000_000_000_000` instead of `1_800_000_000_000_000_000`, the overflow occurs on the very first `buy`, because `reserve1` is initialized to `985_000_000_000_000_000` (`MINT_AMOUNT` minus reserved payouts) and `get_full_token_reserve()` is `18_000_000_000_000_000_000 + 985_000_000_000_000_000 = 18_985_000_000_000_000_000`, while `u64::MAX` is only `18_446_744_073_709_551_615`.

**Zero or near-zero values.** If `v0` is set to zero, `full_sol_reserve = reserve0` with no virtual liquidity depth, causing the bonding curve to price tokens at near-zero cost from the first purchase. If `v1` is set to zero, the token reserve appears depleted before any trading occurs.

Both scenarios affect every subsequent `buy` and `sell` call on missions created after the misconfiguration, and are not recoverable without a config update. Severity is Low because only the trusted owner role can trigger this, but the impact is protocol-wide.

#### Recommended Fix

Add both lower- and upper-bound validation on `v0` and `v1` in both `init` and `set_global_config`. Replace unchecked addition in `get_full_sol_reserve` and `get_full_token_reserve` with `checked_add`.

```rust
require!(v0 >= MIN_VIRTUAL_RESERVE && v0 <= MAX_VIRTUAL_RESERVE, MissionxErrors::InvalidVirtualReserve);
require!(v1 >= MIN_VIRTUAL_RESERVE && v1 <= MAX_VIRTUAL_RESERVE, MissionxErrors::InvalidVirtualReserve);
```

### [F-11] Fee parameters are unbounded in `init` and `set_global_config`

`trade_fee_bps`, `creation_fee`, `fail_fee`, and `migration_fee` are accepted without any upper-bound validation in both `init` and `set_options`.

Setting `trade_fee_bps >= BPS` (10 000) makes the computed fee equal to or greater than the trade amount. In `buy` and `sell`, the fee multiplication uses `checked_mul`, so an excessively large value causes every trade to revert with `MathOverflow`, a protocol-wide DoS on trading. Setting `migration_fee` above the actual `reserve0` balance causes `migrate` to underflow. Setting `creation_fee` to an arbitrary value silently overcharges mission creators.

Severity is Low because only the trusted owner role can trigger this.

#### Recommended Fix

Add upper-bound constraints on `trade_fee_bps`, `migration_fee`, `creation_fee`, and `fail_fee` in both `init` and `set_options`.
