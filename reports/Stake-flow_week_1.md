# Security Audit: StakeFlow Protocol
17.03.26 - 22.03.26

## Acknowledgements
The report is written by [die-kreatur](https://github.com/die-kreatur). It was published before Frank Castle's official judging and represents an independent analysis. 
Since all findings were public, the author reviewed submissions from other participants and included the most notable ones from [0xsophon](https://github.com/0xsophon) and [4Nescient](https://github.com/4Nescient).

## Overview
_Framework: Anchor 0.32 · Token-2022 compatible_

The audited protocol is StakeFlow, which is a dual-mode staking protocol on Solana. Users deposit a base token (`X`) and choose between liquid staking and locked staking.
This audit was performed as part of [Solana Audit Arena — Week 1](https://github.com/Frankcastleauditor/Solana-Audit-Arena), a weekly security competition organized by Frank Castle.

## Scope
All code in `programs/stake-flow/src/lib.rs` (commit 83cf5b3)

## Architecture Summary
The protocol data is stored in a `ProtocolConfig` account, which is a global PDA with all protocol settings and addresses. As a result, the protocol supports only one base token mint for staking.

### Liquid staking
- A user can stake `X` tokens and receive `stX` receipt tokens at a dynamic exchange rate. No lockup. Unstake any time.
- `stX` tokens are minted into the user's wallet on staking and burned on `X` token withdrawal. For the first user, the ratio is 1:1. For subsequent users, the formula is `amount * total_stx_supply / total_staked`.
- Exchange rate for withdrawal: `tokens_out = stx_amount × total_staked / stx_supply`.
- No direct reward claim — yield is captured through the exchange rate.

### Locked staking
- A user can create only one locked staking position (per-user PDA position, `UserStake` account).
- Tokens are locked for 7 days at a higher APR than in liquid staking.
- Rewards accrue continuously from the stake timestamp:
  `rewards = amount × locked_reward_rate_bps × elapsed / (SECONDS_PER_YEAR × 10,000)`,
  where `locked_reward_rate_bps` is set by the admin during protocol initialization and can be updated by the operator at any time (max 50% APR, locked rate must be >= liquid rate).
- Users can claim rewards mid-lockup, instantly exit (forfeiting yield), or partially unstake after lockup with pro-rated yield.
- `UserStake` has a `reward_debt` field that tracks how many rewards have already been paid out. Instead of storing the timestamp of the last claim, the protocol subtracts the already-paid amount from the total accrued rewards to determine the pending payout.

### Protocol management
- A three-role permission system governs the protocol: admin, operator, and liquidity manager.
- Funds from both liquid and locked staking are stored in a shared stake vault `TokenAccount`.
- The liquidity manager can rebalance pools by transferring tokens between the stake vault and the reserve vault. They can also withdraw reserves for yield farming and deposit yield back in.
- Locked staking rewards are paid from the stake vault, not from a separate reward pool.
- The protocol can be paused by the operator or admin. Only the admin can unpause.

### Extra features
- Anyone can donate any SPL token to the protocol via the `donate` instruction. Donated tokens are stored in a per-mint ATA owned by the `ProtocolConfig` PDA.

## Design concerns
- The documentation says users can claim rewards mid-lockup and instantly exit (forfeit yield), but `instant_unlock` doesn't check if any rewards were already paid out. This creates a contradiction and makes locked staking pointless. If users can claim rewards mid-lockup, they can do that and instantly exit with the rewards received at a higher APR.

- The protocol uses `InterfaceAccount` and `TokenInterface`, which means it accepts both standard SPL Token and Token-2022 mints. However, it does not handle Token-2022 extensions (transfer fee, permanent delegate, transfer hook, etc.). For example, a mint with transfer fee would cause accounting desync — the vault receives less than `amount`, but `total_staked` increases by the full `amount`. Since the mint is set once by the admin during initialization, the admin must ensure the chosen mint does not have dangerous extensions.

- The `donate` instruction accepts any SPL token and stores it in a per-mint ATA owned by the protocol, but there is no instruction to withdraw donated tokens. All donations are permanently locked with no way for the admin or anyone else to retrieve them. The purpose of this instruction is unclear, as there is no mechanism to use or withdraw the donated tokens.

## Findings Summary
| ID | Severity | Title |
|----|---------|-------|
| F-01 | Critical | Rewards double claim in `unstake_locked` |
| F-02 | Critical | Reward inflation via rounding mismatch between `claim_rewards` and `partial_unstake` |
| F-03 | High | `deposit_yield` doesn't update `total_staked`, breaking liquid staking yield |
| F-04 | High | Exchange rate manipulation via direct `stX` tokens burn |
| F-05 | Medium | `StakeLiquid` and `UnstakeLiquid` exceed BPF stack limit |
| F-06 | Medium | Reward rate change retroactively affects all existing positions |

## Detailed Findings

### [F-01] Rewards double claim in `unstake_locked`
Locked staking position is stored in the account `UserStake`, which has a `reward_debt` field that tracks how many rewards have already been paid out. Instead of storing the timestamp of the last claim, the protocol subtracts the already-paid amount from the total accrued rewards to determine the pending payout. `unstake_locked` never checks how many rewards have been paid, so it returns money to a user as though they never claimed rewards.

#### Recommended fix
Subtract already-paid rewards from the pending payout:
```rust
let pending_rewards = rewards
    .checked_sub(user_stake.reward_debt)
    .ok_or(StakeFlowError::ArithmeticOverflow)?;

let total_out = amount
    .checked_add(pending_rewards)
    .ok_or(StakeFlowError::ArithmeticOverflow)?;
```

### [F-02] Reward inflation via rounding mismatch between `claim_rewards` and `partial_unstake`
When a user calls `partial_unstake`, the protocol reduces `reward_debt` proportionally to the withdrawn amount: `new_reward_debt = reward_debt × remaining / full_amount`. Due to integer division truncation, the new `reward_debt` is rounded down, which means the protocol "forgets" a portion of already-paid rewards. After that, `claim_rewards` recalculates `total_rewards` from `stake_timestamp` independently and sees `total_rewards - new_reward_debt > 0`, paying out a reward that was already claimed.

This can be repeated in a loop: each cycle of `partial_unstake(1)` followed by `claim_rewards` extracts one extra reward token.

#### Attack vector
1. The attacker stakes 500 tokens at 50% APR (5,000 bps) and waits 7 days for the lockup to expire. Total accrued rewards are 4 tokens (integer truncation of 4.8).
2. The attacker calls `claim_rewards` and receives all 4 accrued tokens. The protocol records `reward_debt = 4`, meaning all rewards have been paid out.
3. The attacker calls `partial_unstake` to withdraw just 1 token. The protocol proportionally scales down the reward debt: `4 × 499 / 500 = 3` (truncated from 3.992). The protocol now thinks only 3 tokens were paid out instead of 4.
4. The attacker immediately calls `claim_rewards`. The protocol recalculates total rewards for the remaining 499 tokens, which still rounds to 4, because the number of rewards is calculated for the total staking period. Since the recorded debt is only 3, the protocol pays out `4 - 3 = 1` token as a "pending" reward, even though all rewards were already claimed in step 2.
5. The attacker repeats: `partial_unstake(1)` drops the debt from 4 back to 3 again due to the same truncation, and `claim_rewards` pays out another unearned token.
6. This loop continues until the position is too small for truncation to produce a difference.

After ~499 cycles, the attacker extracts ~396 extra tokens from 4 legitimate rewards.

#### Recommended fix
Settle all pending rewards before reducing the position, then reset the accounting:
```rust
let pending = total_accrued.checked_sub(user_stake.reward_debt)
    .ok_or(StakeFlowError::ArithmeticOverflow)?;
if pending > 0 { /* transfer pending to user */ }

user_stake.amount = user_stake.amount.checked_sub(unstake_amount)
    .ok_or(StakeFlowError::ArithmeticOverflow)?;
user_stake.stake_timestamp = clock.unix_timestamp;
user_stake.reward_debt = 0;
```

### [F-03] `deposit_yield` doesn't update `total_staked`, breaking liquid staking yield
In accordance with the documentation, `deposit_yield` has to update `total_staked`, but that never happens in the code:
> Rate grows as yield is deposited into the vault and `total_staked` increases

As a result, the exchange rate for liquid staking never grows, so liquid stakers receive no yield.

#### Recommended fix
Update `total_staked` in `deposit_yield` when yield is deposited. Note: `protocol_config` must be marked as `mut` in the `LiquidityManagerAction` context.
```rust
let config = &mut ctx.accounts.protocol_config;
config.total_staked = config
    .total_staked
    .checked_add(amount)
    .ok_or(StakeFlowError::ArithmeticOverflow)?;
```

### [F-04] Exchange rate manipulation via direct `stX` tokens burn
Liquid staking allows a user to deposit their tokens to the stake vault and get `stX` tokens minted directly to their `TokenAccount`. Since tokens are minted to the user's own account, they can do whatever they want with them, including burn. The number of tokens to mint is based on the actual `stX` token supply, which is read directly from the `stX` mint account every time `stake_liquid` is called. This creates an attack surface.

#### Attack vector
1. The attacker stakes tokens in liquid staking mode as the first user, depositing 1001 tokens and receiving 1001 `stX` minted tokens.
2. The attacker burns 1000 `stX` directly via SPL Token. `stx_mint.supply` decreases to 1.
3. A victim stakes 500 tokens.
4. The victim expects to receive approximately 500 `stX` at a fair rate, but the manipulated formula gives `500 * 1 / 1001 = 0` `stX` due to integer truncation. The victim gives X tokens for nothing.
5. Since the victim has 0 `stX` tokens, unstaking is impossible. The X tokens are permanently gone.
6. The attacker calls `unstake_liquid` to return 1 `stX` token and the protocol pays out `1 * 1501 / 1 = 1501`. Attacker's profit is 500 X tokens from the victim's funds.

The attack is not limited to the first user. The larger the `stX` token supply, the more capital an attacker needs to manipulate the rate. Thus, the exploit is most effective for early users.

#### Recommended fix
Mint "dead shares" on the first deposit to prevent the `stX` supply from being reduced to near-zero via direct burns.

### [F-05] `StakeLiquid` and `UnstakeLiquid` exceed BPF stack limit
Solana programs are limited to 4096 bytes of stack space per frame. Exceeding these limits causes a "stack offset exceeded" or "access violation" error, usually when too many variables or large data structures are declared on the stack instead of the heap. Both `StakeLiquid` and `UnstakeLiquid` have 4104 bytes each and an estimated function frame size of 4160 bytes, which exceeds stack limits. Despite the warning, the program can be compiled via `anchor build` and deployed, but no one can call `stake_liquid` or `unstake_liquid` instructions due to stack overflow.

#### Recommended fix
Use heap-allocated data structures for both `StakeLiquid` and `UnstakeLiquid`. Rust has the `Box` smart pointer for this purpose:
```rust
#[derive(Accounts)]
pub struct StakeLiquid<'info> {
    #[account(
        mut,
        seeds = [b"protocol_config"],
        bump = protocol_config.bump,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,

    #[account(
        mut,
        seeds = [b"stx_mint"],
        bump = protocol_config.stx_mint_bump,
        // SECURITY: Verify this is the protocol's stX mint
        constraint = stx_mint.key() == protocol_config.stx_mint @ StakeFlowError::InvalidMint,
    )]
    pub stx_mint: InterfaceAccount<'info, Mint>,

    pub stake_token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [b"stake_vault"],
        bump = protocol_config.stake_vault_bump,
        constraint = stake_vault.key() == protocol_config.stake_vault @ StakeFlowError::InvalidVault,
    )]
    pub stake_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User's token X account (source)
    #[account(
        mut,
        token::mint = stake_token_mint,
        token::authority = user,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User's stX account (destination for receipt tokens)
    #[account(
        mut,
        token::mint = stx_mint,
        token::authority = user,
    )]
    pub user_stx_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}
```

### [F-06] Reward rate change retroactively affects all existing positions
The operator can update reward rates at any time and affect all unpaid rewards retroactively. For example, if some users had a locked staking position for a month and never claimed, and the operator increased the reward rate, those users would get more money than the protocol expected, which could cause vault insolvency. Additionally, a rate decrease after a user has called `claim_rewards` can cause `claim_rewards` and `partial_unstake` to permanently revert. The `reward_debt` was stored at the old higher rate, but total rewards are recomputed at the new lower rate, causing an underflow.

#### Example
1. A user stakes 10,000 tokens at 50% APR (5,000 bps).
2. After 30 days (2,592,000 seconds), `claim_rewards` pays out `10,000 × 5,000 × 2,592,000 / (31,536,000 × 10,000) = 410` tokens. `reward_debt` is set to 410.
3. The operator lowers the rate to 5% APR (500 bps).
4. On the next `claim_rewards` call (day 31, 2,678,400 seconds), total rewards are recomputed from `stake_timestamp` at the new rate: `10,000 × 500 × 2,678,400 / (31,536,000 × 10,000) = 42`.
5. The subtraction `42 - 410` causes an overflow and the transaction reverts.

#### Recommended fix
Use a global accumulated reward index with a per-position checkpoint. When `update_reward_rates` is called, advance the global index by `rate × elapsed / (SECONDS_PER_YEAR × BPS_DENOMINATOR)` and reset the timestamp. Each `UserStake` stores its own `reward_index_at_entry`. Rewards are computed as `amount × (global_index - user.reward_index_at_entry)`. On each claim or partial unstake, the user's index is updated. This correctly isolates pre-change and post-change accrual.
