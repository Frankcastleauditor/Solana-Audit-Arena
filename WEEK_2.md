# Week 2 — MissionX

**Category**: Bounty / Task Marketplace

**Difficulty**: 🔴 Advanced

**Submission deadline**: Sunday March 30, 2026 — 23:59 UTC


---

## Overview

MissionX is an on-chain bounty marketplace built with Anchor and Token-2022. Creators post missions by depositing a SOL payout into a `Missionx` PDA and minting a bonding-curve token tied to that mission. Players accept missions and, upon moderator confirmation, receive the SOL payout and a token allocation. Each mission has its own lifecycle (Unverified → Open → Accepted → Completed/Failed/Withdrawn) and a parallel trading lifecycle (Closed → Open → MigrationRequired → Migrated) that enables speculation on mission outcomes. A dedicated moderator role controls approval, rejection, and outcome adjudication. The protocol also supports a grace period system for failed missions, allowing token holders to sell before funds are fully locked.

---

## Instructions

| Instruction | Description |
|------------|-------------|
| `init` | Owner: initialize the global `Configuration` PDA with protocol parameters |
| `set_global_config` | Owner: update global config parameters (fees, thresholds, payouts, etc.) |
| `update_moderator` | Owner: create or update a `ModeratorState` PDA, enabling or disabling a moderator address |
| `create_missionx` | User: create a new mission — deposits SOL payout, mints a bonding-curve token (Token-2022), initializes the token vault PDA |
| `moderate_initial` | Moderator: review an `Unverified` mission — either approve it (`Open`) or censor it (refunding the creator) |
| `ban_active` | Moderator: block an active mission and optionally ban its trading |
| `unban_active` | Moderator: lift a block on a mission, restoring its previous trade status |
| `switch_ban_to_failed` | Moderator: permanently fail a blocked mission, optionally with immediate fail timestamp |
| `accept_missionx` | Player: accept an `Open` mission — becomes the primary submitter |
| `accept_missionx_multi` | Player: join an already-accepted mission as a secondary submitter (up to 3 total) |
| `complete_missionx` | Moderator or creator: confirm or reject a player's submission — on success, pays out SOL and tokens; on failure, removes player and optionally re-opens |
| `fail_missionx` | Anyone: mark a mission as failed after its `open_duration` expires |
| `buy` | User: buy mission tokens via bonding curve (AMM), with slippage cap and automatic migration trigger |
| `sell` | User: sell mission tokens back to the bonding curve during the grace period |
| `migrate` | Executor: migrate a mission's bonding curve liquidity once the migration threshold is reached |
| `withdraw_from_missionx` | Owner: reclaim SOL and remaining tokens from a failed or expired mission after the grace period |

---

## Accounts

| Account | Type | Description |
|---------|------|-------------|
| `Configuration` | PDA `[b"MISSIONX.CONFIG"]` | Global protocol config: owner, fees, payout bounds, token reserve splits, migration threshold, fail grace period, executor |
| `Missionx` | PDA `[b"MISSIONX.STATE", token_mint]` | Per-mission state: creator, payout amount, mission/trade status, bonding curve reserves (`v0/v1/reserve0/reserve1`), submitters array, migration state, timestamps |
| `ModeratorState` | PDA `[b"MISSIONX.MODERATOR", moderator_pubkey]` | Per-moderator config: enabled flag |
| `token_vault_pda` | PDA `[b"MISSIONX.TOKEN_VAULT", token_mint, missionx_state]` | Token-2022 account owned by `Missionx` PDA — holds the mission's bonding curve token supply |
| `token_mint` | Signer (on creation) / Account | Token-2022 mint created per-mission; mint authority is the `Missionx` PDA |

---

## Roles

MissionX has four distinct roles. Understanding exactly what each role is and isn't allowed to do is critical for finding access control bugs.

| Role | How it's identified | Legitimate permissions |
|------|-------------------|------------------------|
| **Owner** | `config.owner` — set at `init`, updatable via `set_global_config` | Initialize the protocol (`init`); update any global config parameter (`set_global_config`); create/enable/disable moderators (`update_moderator`); withdraw SOL and remaining tokens from any mission in `Open` or `Failed` status once the grace period has elapsed (`withdraw_from_missionx`) |
| **Moderator** | Any address with an enabled `ModeratorState` PDA at `[b"MISSIONX.MODERATOR", moderator_pubkey]` | Approve or censor an `Unverified` mission (`moderate_initial`); block an active mission and optionally ban its trading (`ban_active`); unblock a mission and restore its previous trade status (`unban_active`); permanently fail a blocked mission (`switch_ban_to_failed`); confirm or reject a player's submission on an `Accepted` mission (`complete_missionx`) |
| **Executor** | `config.executor` — set at `init`, updatable via `set_global_config` | Execute the bonding curve migration once `trade_status == MigrationRequired` (`migrate`) — receives the `reserve1` token balance and all accumulated SOL minus the migration fee |
| **Creator** | `missionx_state.creator` — the signer of `create_missionx` | Voluntarily fail their own `Open` mission and recover a partial payout refund (`fail_missionx`); adjudicate a player's submission on their own mission without needing a moderator PDA (`complete_missionx` — the creator path bypasses the `ModeratorState` check entirely) |
| **Player** | Any signer | Accept an `Open` mission as the primary submitter (`accept_missionx`); join an already-accepted mission as a secondary submitter up to 3 total (`accept_missionx_multi`) |
| **Anyone** | Any signer | Buy mission tokens via the bonding curve (`buy`); sell mission tokens back while trading is open (`sell`); trigger time-based mission failure after `open_timestamp + open_duration` has elapsed (`fail_missionx_by_time`) |


## Scope

- All code in `programs/missionx/src/` is in scope
- Focus on: logic bugs, access control, arithmetic, bonding curve manipulation, PDA security, CPI safety, Token-2022 handling, lifecycle state transitions, grace period abuse, moderator trust assumptions
- Out of scope: test files, Anchor framework bugs, Solana runtime issues

---

## How to Submit

1. **Repost** this week's announcement on X + **comment** under it
2. Submit each finding as a **separate GitHub Issue** using the template below

### Issue Title Format

```
[Week 2] [Severity] Short descriptive title
```

**Examples:**
- `[Week 2] [Critical] Attacker can drain payout via re-accepted mission after complete_missionx`
- `[Week 2] [High] Bonding curve reserve underflow on buy near migration threshold`
- `[Week 2] [Medium] Grace period bypass allows sell after fail_ts window`

### Issue Body Template

```markdown
## Finding

**Week**: 2
**Researcher**: [Your GitHub handle + X handle]
**Severity**: [Critical / High / Medium / Low / Informational]
**Category**: [e.g., Missing signer check, Arithmetic overflow, Logic bug, Access control, State machine bypass, etc.]
**Affected function(s)**: [instruction name(s)]

## Description

[Clear explanation of the vulnerability — what's wrong and why it matters]

## Impact

[What can an attacker do? Quantify if possible — e.g., "drain payout SOL from any mission", "bypass moderator approval"]

## Proof of Concept

> REQUIRED — submissions without a PoC will not be scored.

[Provide a concrete PoC that demonstrates the exploit. This can be:]
- A TypeScript/Rust test that triggers the vulnerability
- A step-by-step transaction sequence with exact account setups
- A code diff showing the exploit path with expected vs actual behavior

[The PoC must be detailed enough for independent verification — no guesswork.]

## Recommended Fix

[How to patch it — include a code snippet if possible]
```

---

*Solana Audit Arena — by Frank Castle*
