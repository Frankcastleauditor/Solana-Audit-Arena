# StakeFlow — Liquid & Locked Staking Protocol

> **Solana Audit Arena — Week 1**
> Built with [Safe Solana Builder](https://github.com/Frankcastleauditor/safe-solana-builder) · Framework: Anchor 0.32 · Token-2022 compatible

---

## What is StakeFlow?

StakeFlow is a dual-mode staking protocol on Solana. Users deposit a base token (`X`) and choose between two staking strategies:

- **Liquid staking** — receive `stX` receipt tokens at a dynamic exchange rate. No lockup. Unstake any time.
- **Locked staking** — lock tokens for 7 days at a higher APR. Rewards accrue continuously from the stake timestamp. Users can claim rewards mid-lockup, instantly exit (forfeit yield), or partially unstake after lockup with pro-rated yield.

A three-role permission system governs the protocol. An external liquidity manager can withdraw reserves for yield farming and deposit yield back in, enabling the vault to grow over time.

---

## Architecture

```
ProtocolConfig PDA  [b"protocol_config"]
  └─ admin, operator, liquidity_manager
  └─ stake_token_mint, stx_mint
  └─ stake_vault, reserve_vault (keys)
  └─ liquid_reward_rate_bps, locked_reward_rate_bps
  └─ total_staked, total_locked, total_rewards_distributed
  └─ is_paused, last_update_timestamp

StakeVault           [b"stake_vault"]
  └─ Token account — authority: ProtocolConfig PDA
  └─ Holds all staked token X (liquid + locked)

ReserveVault         [b"reserve_vault"]
  └─ Token account — authority: ProtocolConfig PDA
  └─ Yield/reserve pool managed by liquidity_manager

stX Mint             [b"stx_mint"]
  └─ Receipt token for liquid stakers
  └─ Mint authority + freeze authority: ProtocolConfig PDA

UserStake PDA        [b"user_stake", user_pubkey]
  └─ Per-user locked position (one per wallet)
  └─ amount, stake_timestamp, lockup_expiry, reward_debt, is_active

Donation Vaults      ATA(protocol_config, <any_mint>)
  └─ One Associated Token Account per donated token mint
  └─ Created on first donation via Associated Token program
```

---

## Roles

| Role | Assigned by | Capabilities |
|------|-------------|-------------|
| **Admin** | Self (initializer) | Initialize protocol, set operator, set liquidity manager, unpause |
| **Operator** | Admin | Update reward rates, pause protocol (admin can also call operator functions) |
| **Liquidity Manager** | Admin | Withdraw reserves, deposit yield, rebalance between vaults |
| **User** | Anyone | Stake, unstake, claim rewards, donate |

---

## Instructions

### Admin

| Instruction | Signer | Description |
|-------------|--------|-------------|
| `initialize` | admin | Create all PDAs, vaults, and stX mint. Sets initial liquid/locked reward rates. |
| `set_operator` | admin | Assign a new operator pubkey |
| `set_liquidity_manager` | admin | Assign a new liquidity manager pubkey |
| `unpause_protocol` | admin | Resume protocol after pause |

### Operator / Admin

| Instruction | Signer | Description |
|-------------|--------|-------------|
| `update_reward_rates` | operator or admin | Set new `liquid_reward_rate_bps` and `locked_reward_rate_bps` (max 50% APR = 5000 bps each; locked ≥ liquid) |
| `pause_protocol` | operator or admin | Pause all user-facing instructions |

### Liquidity Manager

| Instruction | Signer | Description |
|-------------|--------|-------------|
| `withdraw_reserves` | liquidity_manager | Pull tokens from reserve vault to manager's account for external farming |
| `deposit_yield` | liquidity_manager | Push tokens back into reserve vault |
| `rebalance_pools` | liquidity_manager | Move tokens between stake vault and reserve vault |

### User

| Instruction | Accounts | Description |
|-------------|----------|-------------|
| `stake_liquid` | user_token_account → stake_vault | Transfer token X, mint stX at `stx_to_mint = amount × stx_supply / total_staked` |
| `unstake_liquid` | user_stx_account → stake_vault | Burn stX, receive `tokens_out = stx_amount × total_staked / stx_supply` |
| `stake_locked` | user_token_account → stake_vault | Transfer token X, create `UserStake` PDA with 7-day lockup |
| `unstake_locked` | stake_vault → user_token_account | After lockup expires: withdraw principal + all accrued locked rewards; close `UserStake` PDA |
| `claim_rewards` | stake_vault → user_token_account | Claim pending locked rewards without unstaking; updates `reward_debt` |
| `instant_unlock` | stake_vault → user_token_account | Exit locked position immediately — **no lockup check** — returns principal only, **no yield**; closes `UserStake` PDA |
| `partial_unstake` | stake_vault → user_token_account | After lockup expires: unstake a portion and collect pro-rated yield; `UserStake` PDA stays open with reduced `amount` and adjusted `reward_debt` |
| `donate` | donor_token_account → donation_vault | Donate any SPL token; vault ATA is created if absent |

---

## Reward Mechanics

### Liquid Staking

- Exchange rate: `tokens_out = stx_amount × total_staked / stx_supply`
- Rate grows as yield is deposited into the vault and `total_staked` increases
- No direct reward claim — yield is captured through the exchange rate

### Locked Staking

Rewards accrue linearly from `stake_timestamp`:

```
total_accrued = amount × locked_reward_rate_bps × elapsed_seconds
                ──────────────────────────────────────────────────
                         SECONDS_PER_YEAR × BPS_DENOMINATOR
```

`reward_debt` tracks rewards already paid out. Pending at any time = `total_accrued - reward_debt`.

**Partial unstake** collects `pending × unstake_amount / full_amount` and scales `reward_debt` proportionally for the remaining position.

---

## Constants

| Constant | Value | Meaning |
|----------|-------|---------|
| `LOCKUP_DURATION` | 604,800 s (7 days) | Locked staking lockup period |
| `BPS_DENOMINATOR` | 10,000 | Basis points denominator |
| `MAX_REWARD_RATE_BPS` | 5,000 | Maximum APR = 50% |
| `SECONDS_PER_YEAR` | 31,536,000 | Used in reward rate calculation |

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

Program ID: `B2jG5ZT7ySJmqpiWx3Jpszsy3LKtsLy9dxS8jvgjaBuq`

---

## Security Notes

- All arithmetic uses `checked_*` operations with `u128` intermediaries to prevent overflow
- PDA seeds include the user's pubkey where applicable to prevent cross-user account substitution
- Vault accounts are validated by key against stored values in `ProtocolConfig`
- The stX mint authority is exclusively the `ProtocolConfig` PDA
- Protocol can be paused by operator or admin; only admin can unpause
- Reward rates are bounded at 50% APR
- Locked reward rate must always be ≥ liquid reward rate

---

## Audit Competition

This program is the **Week 1** target of the [Solana Audit Arena](https://github.com/Frankcastleauditor/Solana-Audit-Arena) — a free weekly security competition judged by Frank Castle ([@0xcastle_chain](https://x.com/0xcastle_chain)).

**Submission deadline**: Sunday March 22, 2026 — 23:59 UTC

See [WEEK_1.md](./WEEK_1.md) for this week's brief and [competition rules](https://github.com/Frankcastleauditor/Solana-Audit-Arena/blob/main/README.md) for the submission format.

---

*Solana Audit Arena — by Frank Castle. Securing Solana, one researcher at a time.*
