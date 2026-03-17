# Week 1 — StakeFlow

**Category**: Staking

**Difficulty**: 🟡 Intermediate

**Submission deadline**: Sunday March 22, 2026 — 23:59 UTC

**commit Hash** : https://github.com/Frankcastleauditor/Solana-Audit-Arena/commit/83cf5b3c58bd6a84878032eb5bc86953a8bb5885

---

## Overview

StakeFlow is a dual-mode liquid and locked staking protocol built with Anchor and Token-2022 support. Users can stake a base token (`X`) in either **liquid mode** (receiving tradeable `stX` receipt tokens at a dynamic exchange rate) or **locked mode** (7-day lockup, higher APR, per-user PDA position). A three-role permission model — admin, operator, and liquidity manager — governs protocol configuration, rate adjustments, and reserve management. The protocol also supports permissionless donations of any SPL token, emergency instant unlock, and partial unstaking with pro-rated yield collection.

## Instructions

| Instruction | Description |
|------------|-------------|
| `initialize` | Admin: deploy ProtocolConfig PDA, stake vault, reserve vault, and stX mint in one transaction |
| `set_operator` | Admin: assign or replace the operator role |
| `set_liquidity_manager` | Admin: assign or replace the liquidity manager role |
| `stake_liquid` | User: transfer token X to stake vault, receive stX at current exchange rate |
| `stake_locked` | User: transfer token X to stake vault, create a `UserStake` PDA with 7-day lockup and bonus APR |
| `unstake_liquid` | User: burn stX tokens, receive token X back proportional to the current exchange rate |
| `unstake_locked` | User: after lockup expires, withdraw principal + accrued locked rewards; closes `UserStake` PDA |
| `claim_rewards` | User: claim pending rewards from an active locked position without unstaking |
| `instant_unlock` | User: exit a locked position at any time — no lockup check — principal returned, no yield |
| `partial_unstake` | User: after lockup expires, unstake a portion of a locked position and collect pro-rated yield; position remains open with reduced amount |
| `update_reward_rates` | Operator/Admin: update liquid and locked APR reward rates (bounded by `MAX_REWARD_RATE_BPS = 5000`) |
| `pause_protocol` | Operator/Admin: emergency pause — blocks all user-facing instructions |
| `unpause_protocol` | Admin only: unpause the protocol |
| `withdraw_reserves` | Liquidity Manager: withdraw tokens from the reserve vault for external yield farming |
| `deposit_yield` | Liquidity Manager: deposit yield back into the reserve vault |
| `rebalance_pools` | Liquidity Manager: move tokens between the stake vault and the reserve vault in either direction |
| `donate` | Anyone: donate any SPL token to the protocol; the per-mint ATA vault is created via the Associated Token program if it does not yet exist |

## Accounts

| Account | Type | Description |
|---------|------|-------------|
| `ProtocolConfig` | PDA `[b"protocol_config"]` | Global protocol state: admin/operator/liq_manager keys, vault addresses, stX mint address, reward rates, total staked/locked/distributed, pause flag, bump seeds |
| `StakeVault` | PDA `[b"stake_vault"]` | Protocol-owned token account holding all staked token X (serves both liquid and locked stakers) |
| `ReserveVault` | PDA `[b"reserve_vault"]` | Protocol-owned token account for yield reserves; liquidity manager can withdraw/deposit here |
| `stX Mint` | PDA `[b"stx_mint"]` | Liquid staking receipt token mint — mint and freeze authority is `ProtocolConfig`; supply tracks the liquid pool |
| `UserStake` | PDA `[b"user_stake", user_pubkey]` | Per-user locked staking position: staked amount, timestamps, lockup expiry, reward debt, active flag |
| Donation Vault | ATA (`protocol_config` authority, per donated mint) | Associated token account auto-initialized by the `donate` instruction for any token mint |

## Scope

- All code in `programs/stake-flow/src/lib.rs` is in scope
- Focus on: logic bugs, access control, arithmetic, PDA security, CPI safety, Token-2022 handling, reward accounting


## How to Submit

1. **Repost** this week's announcement on X + **comment** under it
2. Submit each finding as a **separate GitHub Issue** using the [submission template](../README.md#issue-body-template)

---

*Solana Audit Arena — by Frank Castle*
