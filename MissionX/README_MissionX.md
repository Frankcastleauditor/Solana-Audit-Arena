# MissionX — On-Chain Bounty Marketplace

> **Solana Audit Arena — Week 2**
> Built with [Safe Solana Builder](https://github.com/Frankcastleauditor/safe-solana-builder) · Framework: Anchor 0.32 · Token-2022 compatible

---

## What is MissionX?

MissionX is an on-chain bounty marketplace on Solana. Creators post missions by depositing a SOL payout into a `Missionx` PDA. Each mission automatically mints its own Token-2022 bonding-curve token, enabling open speculation on mission outcomes before they resolve.

The mission lifecycle has two parallel state machines:

- **Mission status** — `Unverified → Open → Accepted → Completed / Failed / Withdrawn` — governed by moderators and creators
- **Trade status** — `Closed → Open → MigrationRequired → Migrated` — driven by bonding curve activity

A moderator must approve a mission before players can accept it. Once a player completes the mission and a moderator (or the creator) confirms success, the player receives the SOL payout and a token allocation. If the mission expires or the creator cancels, token holders get a grace period to sell before the protocol closes trading.

---

## Architecture

```
Configuration PDA     [b"MISSIONX.CONFIG"]
  └─ owner, executor, fee_recipient
  └─ is_enabled, missionx_payout_min/max
  └─ creation_fee, trade_fee_bps
  └─ v0, v1 (initial bonding curve virtual reserves)
  └─ token_player_payout, token_creator_payout
  └─ migration_threshold, migration_fee
  └─ fail_grace_period, fail_fee
  └─ metadata_authority, ipns_root

Missionx PDA          [b"MISSIONX.STATE", token_mint]
  └─ creator, payout_amount
  └─ missionx_status, trade_status, is_blocked
  └─ token_mint, token_program
  └─ v0, v1 (virtual reserves — copied from config at creation)
  └─ reserve0 (accumulated SOL), reserve1 (remaining tokens)
  └─ submitters: [Option<Pubkey>; 3]
  └─ token_player_payout, token_creator_payout
  └─ migration_threshold, migration_fee
  └─ open_timestamp, open_duration, success_time
  └─ fail_ts, old_trade_status

ModeratorState PDA    [b"MISSIONX.MODERATOR", moderator_pubkey]
  └─ is_enabled

Token Vault PDA       [b"MISSIONX.TOKEN_VAULT", token_mint, missionx_state]
  └─ Token-2022 account — authority: Missionx PDA
  └─ Holds the mission's bonding curve token supply

Token Mint            (signer at creation, then normal account)
  └─ Token-2022 mint created per-mission
  └─ Mint authority: Missionx PDA
  └─ 1,000,000,000 × 10^9 tokens minted at creation
```

---

## Roles

| Role | Assigned by | Capabilities |
|------|-------------|-------------|
| **Owner** | Self (initializer) | Initialize protocol, update global config, manage moderators, withdraw from expired/failed missions |
| **Moderator** | Owner (via `update_moderator`) | Approve/censor missions, ban/unban active missions, fail blocked missions, adjudicate player submissions |
| **Executor** | Owner (set in config) | Trigger bonding curve migration once migration threshold is reached |
| **Creator** | Anyone (signer of `create_missionx`) | Post missions, voluntarily fail their own open mission, adjudicate their own mission without a moderator PDA |
| **Player** | Anyone | Accept open missions, join as secondary submitter (up to 3 per mission) |
| **Anyone** | — | Buy/sell mission tokens via bonding curve, trigger time-based mission failure after expiry |

---

## Instructions

### Owner

| Instruction | Signer | Description |
|-------------|--------|-------------|
| `init` | owner | Initialize the global `Configuration` PDA with all protocol parameters |
| `set_global_config` | owner | Update any config parameter: fees, payout bounds, reward splits, migration settings, grace period, executor, etc. |
| `update_moderator` | owner | Create or update a `ModeratorState` PDA — enables or disables a moderator address |
| `withdraw_from_missionx` | owner | Reclaim SOL and remaining tokens from an `Open` or `Failed` mission once the grace period has elapsed |

### Moderator

| Instruction | Signer | Description |
|-------------|--------|-------------|
| `moderate_initial` | moderator | Approve (`Open`) or censor (`Censored`) an `Unverified` mission; censoring refunds the creator's payout minus `fail_fee` |
| `ban_active` | moderator | Block an active mission; optionally freeze its trading by setting `trade_status = Banned` |
| `unban_active` | moderator | Lift a block and restore the mission's previous trade status |
| `switch_ban_to_failed` | moderator | Permanently fail a blocked mission; optionally set `fail_ts = 0` for an immediate grace period start |
| `complete_missionx` | moderator or creator | Confirm or reject a player's submission on an `Accepted` mission |

### Executor

| Instruction | Signer | Description |
|-------------|--------|-------------|
| `migrate` | executor | Execute bonding curve migration when `trade_status == MigrationRequired` — transfers `reserve1` tokens and accumulated `reserve0` SOL (minus `migration_fee`) to the executor; distributes token payouts to player and creator |

### Creator

| Instruction | Signer | Description |
|-------------|--------|-------------|
| `create_missionx` | creator | Post a new mission — deposits SOL payout, pays creation fee, mints a Token-2022 bonding-curve token, initializes the token vault |
| `fail_missionx` | creator | Voluntarily cancel an `Open` mission — triggers payout refund (minus `fail_fee`) back to creator |

### Player

| Instruction | Signer | Description |
|-------------|--------|-------------|
| `accept_missionx` | player | Accept an `Open` mission as the primary submitter; sets `trade_status = Open` |
| `accept_missionx_multi` | player | Join an already-`Accepted` mission as a secondary submitter (slots 2–3) |

### Anyone

| Instruction | Accounts | Description |
|-------------|----------|-------------|
| `buy` | user → missionx_state (SOL), token_vault → user_ata (tokens) | Buy mission tokens via bonding curve with a SOL `pay_cap` slippage guard; automatically sets `trade_status = MigrationRequired` when `reserve0` reaches `migration_threshold` |
| `sell` | user_ata → token_vault (tokens), missionx_state → user (SOL) | Sell mission tokens back to the bonding curve with a `min_out` slippage guard; available during the grace period of failed missions |
| `fail_missionx_by_time` | any signer | Mark an `Open` mission as `Failed` once `open_timestamp + open_duration < clock.unix_timestamp`; sets `fail_ts` to the expiry timestamp and refunds the creator |

---

## Mission Lifecycle

```
                      ┌─────────────┐
                      │  Unverified │  (created by anyone)
                      └──────┬──────┘
               moderate_initial()
              ┌───────────────┴───────────────┐
           block=false                     block=true
              │                               │
        ┌─────▼──────┐                 ┌──────▼──────┐
        │    Open    │                 │  Censored   │
        └─────┬──────┘                 └─────────────┘
   accept_missionx()
        │
   ┌────▼────────┐
   │  Accepted   │
   └────┬────────┘
  complete_missionx()
   ┌────┴────────────────┐
is_successful=true    is_successful=false (last player)
   │                      │
┌──▼──────┐         ┌─────▼──┐
│Completed│         │  Open  │  (re-opened for new player)
└─────────┘         └────────┘

fail_missionx() / fail_missionx_by_time() / switch_ban_to_failed()
        │
   ┌────▼──────┐
   │  Failed   │  (grace period — trading still open)
   └────┬──────┘
  withdraw_from_missionx() (after grace period)
        │
   ┌────▼──────────┐
   │   Withdrawn   │
   └───────────────┘
```

---

## Bonding Curve Mechanics

Each mission has its own constant-product AMM using the formula:

```
amount_out = amount_in × reserve_out / (amount_in + reserve_in)
```

The full reserve used in pricing is split into virtual and real components:

```
full_sol_reserve   = v0 + reserve0   (virtual SOL + accumulated SOL)
full_token_reserve = v1 + reserve1   (virtual tokens + remaining tokens)
```

`v0` and `v1` are copied from the global config at creation and set the initial price. `reserve0` and `reserve1` move with each trade.

**Migration** is triggered when `reserve0 >= migration_threshold`. At that point:
- `trade_status` is set to `MigrationRequired`
- The executor calls `migrate`, which transfers all `reserve1` tokens and `reserve0` SOL to the executor (minus `migration_fee` to the fee recipient)
- Token payouts (`token_player_payout`, `token_creator_payout`) are distributed to the player and creator at migration time

**Trading fee** is charged on each buy and sell as `sol_amount × trade_fee_bps / 10_000`, paid to `fee_recipient`.

---

## Constants

| Constant | Value | Meaning |
|----------|-------|---------|
| `MINT_AMOUNT` | 1,000,000,000 × 10^9 | Total tokens minted per mission at creation |
| `BPS` | 10,000 | Basis points denominator |
| `MISSIONX_OPEN_DURATION_MIN` | 3,600 s (1 hour) | Minimum mission open window |
| `MISSIONX_OPEN_DURATION_MAX` | 172,800 s (48 hours) | Maximum mission open window |

---

## Building & Testing

**Requirements**: Solana CLI, Anchor 0.32, Yarn

```bash
# Install dependencies
yarn install

# Build
anchor build

# Run tests
anchor test
```

Program ID: *(set at deployment)*

---

## Security Notes

- All arithmetic uses `checked_*` operations and `u128` intermediaries to prevent overflow
- The token vault PDA is derived from both `token_mint` and `missionx_state` — preventing cross-mission account substitution
- `token_program` is stored in `Missionx` state and validated at the account level via `address = missionx_state.token_program` on every instruction that performs a CPI
- The bonding curve virtual reserves (`v0`, `v1`) are copied from global config into each `Missionx` at creation — global config changes do not retroactively affect live missions
- `fail_grace_period` is a global config value — the owner can modify it, affecting the withdrawal window for all active missions simultaneously
- A moderator PDA is required for all moderation actions except `complete_missionx` when called by the creator — the creator path bypasses the `ModeratorState` check entirely
- No check prevents a creator from also holding a `ModeratorState` PDA and self-approving their own mission via `moderate_initial`
- No check prevents the creator from accepting their own mission as a player via `accept_missionx`

---

## Audit Competition

This program is the **Week 2** target of the [Solana Audit Arena](https://github.com/Frankcastleauditor/Solana-Audit-Arena) — a free weekly security competition judged by Frank Castle ([@0xcastle_chain](https://x.com/0xcastle_chain)).

**Submission deadline**: Sunday March 30, 2026 — 23:59 UTC

See [WEEK_2.md](./WEEK_2.md) for this week's brief and [competition rules](https://github.com/Frankcastleauditor/Solana-Audit-Arena/blob/main/README.md) for the submission format.

---

*Solana Audit Arena — by Frank Castle. Securing Solana, one researcher at a time.*
