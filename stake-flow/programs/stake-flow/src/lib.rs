// ============================================================
// Program: StakeFlow — Liquid & Locked Staking Protocol
// Framework: Anchor
// Author: Frank Castle Security Template
//
// Architecture:
//   ProtocolConfig PDA  — global config (admin, operator, liq_manager, rates)
//   StakeVault PDA      — holds staked tokens (program-controlled)
//   ReserveVault PDA    — holds reserve/yield tokens (program-controlled)
//   UserStake PDA       — per-user staking position (liquid or locked)
//   stX Mint PDA        — liquid staking receipt token mint
//
// Roles:
//   Admin            — initializes protocol, sets operator/liq_manager, emergency pause
//   Operator         — adjusts reward rates, pauses/unpauses
//   Liquidity Manager— withdraws reserves for yield farming, deposits yield back
//   User             — stakes, unstakes, claims rewards
//
// Instructions:
//   initialize               — admin: create ProtocolConfig + vaults + stX mint
//   set_operator              — admin: set/change operator pubkey
//   set_liquidity_manager     — admin: set/change liquidity manager pubkey
//   stake_liquid              — user: stake token X, receive stX
//   stake_locked              — user: stake token X with 7-day lockup for bonus rewards
//   unstake_liquid            — user: burn stX, receive token X back
//   unstake_locked            — user: withdraw locked stake after lockup expires
//   claim_rewards             — user: claim accrued rewards
//   update_reward_rates       — operator: adjust liquid/locked reward rates
//   pause_protocol            — operator/admin: emergency pause
//   unpause_protocol          — admin: unpause
//   withdraw_reserves         — liq_manager: withdraw from reserve for yield farming
//   deposit_yield             — liq_manager: deposit yield back into protocol
//   rebalance_pools           — liq_manager: move tokens between stake and reserve vaults
// ============================================================

// ─────────────────────────────────────────────────────────────
// CONSTANTS
// ─────────────────────────────────────────────────────────────

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, Burn, Mint, MintTo, TokenAccount, TokenInterface, TransferChecked,
};
declare_id!("B2jG5ZT7ySJmqpiWx3Jpszsy3LKtsLy9dxS8jvgjaBuq");

/// Lockup duration for locked staking: 7 days in seconds
const LOCKUP_DURATION: i64 = 7 * 24 * 60 * 60;

/// Basis points denominator (10_000 = 100%)
const BPS_DENOMINATOR: u64 = 10_000;

/// Maximum reward rate: 50% APR in basis points
const MAX_REWARD_RATE_BPS: u64 = 5_000;

/// Seconds per year (approximate) for reward calculation
const SECONDS_PER_YEAR: u64 = 365 * 24 * 60 * 60;

// ─────────────────────────────────────────────────────────────
// PROGRAM
// ─────────────────────────────────────────────────────────────

#[program]
pub mod stake_flow {
    use super::*;

    /// Initialize the protocol. Can only be called once — `init` enforces this.
    /// Creates the ProtocolConfig, stake vault, reserve vault, and stX mint.
    pub fn initialize(
        ctx: Context<Initialize>,
        liquid_reward_rate_bps: u64,
        locked_reward_rate_bps: u64,
    ) -> Result<()> {
        // SECURITY: Validate reward rates are within bounds
        require!(
            liquid_reward_rate_bps <= MAX_REWARD_RATE_BPS,
            StakeFlowError::RewardRateTooHigh
        );
        require!(
            locked_reward_rate_bps <= MAX_REWARD_RATE_BPS,
            StakeFlowError::RewardRateTooHigh
        );
        require!(
            locked_reward_rate_bps >= liquid_reward_rate_bps,
            StakeFlowError::LockedRateMustExceedLiquid
        );

        let config = &mut ctx.accounts.protocol_config;
        config.admin = ctx.accounts.admin.key();
        config.operator = ctx.accounts.admin.key(); // Default: admin is operator until changed
        config.liquidity_manager = ctx.accounts.admin.key(); // Default: admin until changed
        config.stake_token_mint = ctx.accounts.stake_token_mint.key();
        config.stx_mint = ctx.accounts.stx_mint.key();
        config.stake_vault = ctx.accounts.stake_vault.key();
        config.reserve_vault = ctx.accounts.reserve_vault.key();
        config.liquid_reward_rate_bps = liquid_reward_rate_bps;
        config.locked_reward_rate_bps = locked_reward_rate_bps;
        config.total_staked = 0;
        config.total_locked = 0;
        config.total_rewards_distributed = 0;
        config.is_paused = false;
        config.last_update_timestamp = Clock::get()?.unix_timestamp;
        config.bump = ctx.bumps.protocol_config;
        config.stake_vault_bump = ctx.bumps.stake_vault;
        config.reserve_vault_bump = ctx.bumps.reserve_vault;
        config.stx_mint_bump = ctx.bumps.stx_mint;

        emit!(ProtocolInitialized {
            admin: config.admin,
            liquid_rate: liquid_reward_rate_bps,
            locked_rate: locked_reward_rate_bps,
        });

        Ok(())
    }

    /// Admin sets/changes the operator. Only admin can call this.
    pub fn set_operator(ctx: Context<AdminOnly>, new_operator: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.protocol_config;
        let old_operator = config.operator;
        config.operator = new_operator;

        emit!(OperatorChanged {
            old_operator,
            new_operator,
        });

        Ok(())
    }

    /// Admin sets/changes the liquidity manager. Only admin can call this.
    pub fn set_liquidity_manager(ctx: Context<AdminOnly>, new_manager: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.protocol_config;
        let old_manager = config.liquidity_manager;
        config.liquidity_manager = new_manager;

        emit!(LiquidityManagerChanged {
            old_manager,
            new_manager,
        });

        Ok(())
    }

    /// User stakes tokens in liquid mode. Receives stX tokens at current exchange rate.
    /// No lockup — can unstake at any time.
    pub fn stake_liquid(ctx: Context<StakeLiquid>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.protocol_config;

        // SECURITY: Protocol must not be paused
        require!(!config.is_paused, StakeFlowError::ProtocolPaused);
        require!(amount > 0, StakeFlowError::ZeroAmount);

        // Calculate stX tokens to mint based on exchange rate
        // exchange_rate = total_staked / total_stx_supply
        // stx_to_mint = amount / exchange_rate = amount * total_stx_supply / total_staked
        let stx_supply = ctx.accounts.stx_mint.supply;
        let stx_to_mint = if stx_supply == 0 || config.total_staked == 0 {
            // First staker: 1:1 ratio
            amount
        } else {
            // SECURITY: Multiply before divide to preserve precision
            (amount as u128)
                .checked_mul(stx_supply as u128)
                .ok_or(StakeFlowError::ArithmeticOverflow)?
                .checked_div(config.total_staked as u128)
                .ok_or(StakeFlowError::ArithmeticOverflow)? as u64
        };

        require!(stx_to_mint > 0, StakeFlowError::AmountTooSmall);

        // Transfer tokens from user to stake vault
        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    mint: ctx.accounts.stake_token_mint.to_account_info(),
                    to: ctx.accounts.stake_vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount,
            ctx.accounts.stake_token_mint.decimals,
        )?;

        // Mint stX tokens to user
        let config_seeds = &[b"protocol_config".as_ref(), &[config.bump]];
        let signer_seeds = &[&config_seeds[..]];

        token_interface::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.stx_mint.to_account_info(),
                    to: ctx.accounts.user_stx_account.to_account_info(),
                    authority: ctx.accounts.protocol_config.to_account_info(),
                },
                signer_seeds,
            ),
            stx_to_mint,
        )?;

        // Update protocol state
        let config = &mut ctx.accounts.protocol_config;
        config.total_staked = config
            .total_staked
            .checked_add(amount)
            .ok_or(StakeFlowError::ArithmeticOverflow)?;

        emit!(LiquidStaked {
            user: ctx.accounts.user.key(),
            amount,
            stx_minted: stx_to_mint,
        });

        Ok(())
    }

    /// User stakes tokens in locked mode. Creates a UserStake PDA with lockup expiry.
    /// Higher rewards but tokens are locked for LOCKUP_DURATION.
    pub fn stake_locked(ctx: Context<StakeLocked>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.protocol_config;

        require!(!config.is_paused, StakeFlowError::ProtocolPaused);
        require!(amount > 0, StakeFlowError::ZeroAmount);

        let clock = Clock::get()?;

        // Transfer tokens from user to stake vault
        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    mint: ctx.accounts.stake_token_mint.to_account_info(),
                    to: ctx.accounts.stake_vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount,
            ctx.accounts.stake_token_mint.decimals,
        )?;

        // Initialize user stake position
        let user_stake = &mut ctx.accounts.user_stake;
        user_stake.user = ctx.accounts.user.key();
        user_stake.amount = amount;
        user_stake.stake_timestamp = clock.unix_timestamp;
        // SECURITY: Lockup expiry calculated with checked math
        user_stake.lockup_expiry = clock
            .unix_timestamp
            .checked_add(LOCKUP_DURATION)
            .ok_or(StakeFlowError::ArithmeticOverflow)?;
        user_stake.reward_debt = 0;
        user_stake.is_active = true;
        user_stake.bump = ctx.bumps.user_stake;

        // Update protocol state
        let config = &mut ctx.accounts.protocol_config;
        config.total_locked = config
            .total_locked
            .checked_add(amount)
            .ok_or(StakeFlowError::ArithmeticOverflow)?;

        emit!(LockedStaked {
            user: ctx.accounts.user.key(),
            amount,
            lockup_expiry: user_stake.lockup_expiry,
        });

        Ok(())
    }

    /// User unstakes liquid position. Burns stX tokens, receives token X back.
    pub fn unstake_liquid(ctx: Context<UnstakeLiquid>, stx_amount: u64) -> Result<()> {
        let config = &ctx.accounts.protocol_config;

        require!(!config.is_paused, StakeFlowError::ProtocolPaused);
        require!(stx_amount > 0, StakeFlowError::ZeroAmount);

        // Calculate token X to return based on exchange rate
        // tokens_out = stx_amount * total_staked / total_stx_supply
        let stx_supply = ctx.accounts.stx_mint.supply;
        require!(stx_supply > 0, StakeFlowError::NoLiquidity);

        // SECURITY: Multiply before divide
        let tokens_out = (stx_amount as u128)
            .checked_mul(config.total_staked as u128)
            .ok_or(StakeFlowError::ArithmeticOverflow)?
            .checked_div(stx_supply as u128)
            .ok_or(StakeFlowError::ArithmeticOverflow)? as u64;

        require!(tokens_out > 0, StakeFlowError::AmountTooSmall);

        // Verify stake vault has enough tokens
        require!(
            ctx.accounts.stake_vault.amount >= tokens_out,
            StakeFlowError::InsufficientVaultBalance
        );

        // Burn stX tokens from user
        token_interface::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.stx_mint.to_account_info(),
                    from: ctx.accounts.user_stx_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            stx_amount,
        )?;

        // Transfer token X from vault to user
        let config_seeds = &[b"protocol_config".as_ref(), &[config.bump]];
        let signer_seeds = &[&config_seeds[..]];

        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.stake_vault.to_account_info(),
                    mint: ctx.accounts.stake_token_mint.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.protocol_config.to_account_info(),
                },
                signer_seeds,
            ),
            tokens_out,
            ctx.accounts.stake_token_mint.decimals,
        )?;

        // Update protocol state
        let config = &mut ctx.accounts.protocol_config;
        config.total_staked = config
            .total_staked
            .checked_sub(tokens_out)
            .ok_or(StakeFlowError::ArithmeticOverflow)?;

        emit!(LiquidUnstaked {
            user: ctx.accounts.user.key(),
            stx_burned: stx_amount,
            tokens_returned: tokens_out,
        });

        Ok(())
    }

    /// User unstakes locked position after lockup period expires.
    /// Closes the UserStake PDA and returns staked tokens + accrued rewards.
    pub fn unstake_locked(ctx: Context<UnstakeLocked>) -> Result<()> {
        let config = &ctx.accounts.protocol_config;
        let user_stake = &ctx.accounts.user_stake;

        require!(!config.is_paused, StakeFlowError::ProtocolPaused);
        require!(user_stake.is_active, StakeFlowError::StakeNotActive);

        // SECURITY: Enforce lockup period
        let clock = Clock::get()?;
        require!(
            clock.unix_timestamp >= user_stake.lockup_expiry,
            StakeFlowError::LockupNotExpired
        );

        let amount = user_stake.amount;

        // Calculate rewards: amount * rate * duration / (SECONDS_PER_YEAR * BPS_DENOMINATOR)
        let duration = (clock
            .unix_timestamp
            .checked_sub(user_stake.stake_timestamp)
            .ok_or(StakeFlowError::ArithmeticOverflow)?) as u64;

        // SECURITY: Multiply before divide, use u128 to avoid overflow
        let rewards = (amount as u128)
            .checked_mul(config.locked_reward_rate_bps as u128)
            .ok_or(StakeFlowError::ArithmeticOverflow)?
            .checked_mul(duration as u128)
            .ok_or(StakeFlowError::ArithmeticOverflow)?
            .checked_div(
                (SECONDS_PER_YEAR as u128)
                    .checked_mul(BPS_DENOMINATOR as u128)
                    .ok_or(StakeFlowError::ArithmeticOverflow)?,
            )
            .ok_or(StakeFlowError::ArithmeticOverflow)? as u64;

        let total_out = amount
            .checked_add(rewards)
            .ok_or(StakeFlowError::ArithmeticOverflow)?;

        // Verify vault has enough
        require!(
            ctx.accounts.stake_vault.amount >= total_out,
            StakeFlowError::InsufficientVaultBalance
        );

        // Transfer tokens + rewards from vault to user
        let config_seeds = &[b"protocol_config".as_ref(), &[config.bump]];
        let signer_seeds = &[&config_seeds[..]];

        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.stake_vault.to_account_info(),
                    mint: ctx.accounts.stake_token_mint.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.protocol_config.to_account_info(),
                },
                signer_seeds,
            ),
            total_out,
            ctx.accounts.stake_token_mint.decimals,
        )?;

        // Update protocol state
        let config = &mut ctx.accounts.protocol_config;
        config.total_locked = config
            .total_locked
            .checked_sub(amount)
            .ok_or(StakeFlowError::ArithmeticOverflow)?;
        config.total_rewards_distributed = config
            .total_rewards_distributed
            .checked_add(rewards)
            .ok_or(StakeFlowError::ArithmeticOverflow)?;

        // NOTE: UserStake account is closed via `close = user` constraint

        emit!(LockedUnstaked {
            user: ctx.accounts.user.key(),
            amount,
            rewards,
        });

        Ok(())
    }

    /// User claims accrued rewards for their locked position without unstaking.
    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        let config = &ctx.accounts.protocol_config;
        let user_stake = &mut ctx.accounts.user_stake;

        require!(!config.is_paused, StakeFlowError::ProtocolPaused);
        require!(user_stake.is_active, StakeFlowError::StakeNotActive);

        let clock = Clock::get()?;

        // Calculate pending rewards
        let duration = (clock
            .unix_timestamp
            .checked_sub(user_stake.stake_timestamp)
            .ok_or(StakeFlowError::ArithmeticOverflow)?) as u64;

        // SECURITY: u128 intermediary, multiply before divide
        let total_rewards = (user_stake.amount as u128)
            .checked_mul(config.locked_reward_rate_bps as u128)
            .ok_or(StakeFlowError::ArithmeticOverflow)?
            .checked_mul(duration as u128)
            .ok_or(StakeFlowError::ArithmeticOverflow)?
            .checked_div(
                (SECONDS_PER_YEAR as u128)
                    .checked_mul(BPS_DENOMINATOR as u128)
                    .ok_or(StakeFlowError::ArithmeticOverflow)?,
            )
            .ok_or(StakeFlowError::ArithmeticOverflow)? as u64;

        let pending_rewards = total_rewards
            .checked_sub(user_stake.reward_debt)
            .ok_or(StakeFlowError::ArithmeticOverflow)?;

        require!(pending_rewards > 0, StakeFlowError::NoRewardsToClaim);

        // Verify vault has enough
        require!(
            ctx.accounts.stake_vault.amount >= pending_rewards,
            StakeFlowError::InsufficientVaultBalance
        );

        // Transfer rewards from vault to user
        let config_seeds = &[b"protocol_config".as_ref(), &[config.bump]];
        let signer_seeds = &[&config_seeds[..]];

        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.stake_vault.to_account_info(),
                    mint: ctx.accounts.stake_token_mint.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.protocol_config.to_account_info(),
                },
                signer_seeds,
            ),
            pending_rewards,
            ctx.accounts.stake_token_mint.decimals,
        )?;

        // Update reward debt
        user_stake.reward_debt = total_rewards;

        // Update protocol state
        let config = &mut ctx.accounts.protocol_config;
        config.total_rewards_distributed = config
            .total_rewards_distributed
            .checked_add(pending_rewards)
            .ok_or(StakeFlowError::ArithmeticOverflow)?;

        emit!(RewardsClaimed {
            user: ctx.accounts.user.key(),
            amount: pending_rewards,
        });

        Ok(())
    }

    /// Operator adjusts reward rates. Bounded by MAX_REWARD_RATE_BPS.
    pub fn update_reward_rates(
        ctx: Context<OperatorOnly>,
        new_liquid_rate_bps: u64,
        new_locked_rate_bps: u64,
    ) -> Result<()> {
        require!(
            new_liquid_rate_bps <= MAX_REWARD_RATE_BPS,
            StakeFlowError::RewardRateTooHigh
        );
        require!(
            new_locked_rate_bps <= MAX_REWARD_RATE_BPS,
            StakeFlowError::RewardRateTooHigh
        );
        require!(
            new_locked_rate_bps >= new_liquid_rate_bps,
            StakeFlowError::LockedRateMustExceedLiquid
        );

        let config = &mut ctx.accounts.protocol_config;
        config.liquid_reward_rate_bps = new_liquid_rate_bps;
        config.locked_reward_rate_bps = new_locked_rate_bps;
        config.last_update_timestamp = Clock::get()?.unix_timestamp;

        emit!(RewardRatesUpdated {
            liquid_rate: new_liquid_rate_bps,
            locked_rate: new_locked_rate_bps,
        });

        Ok(())
    }

    /// Operator or admin pauses the protocol. Blocks all user operations.
    pub fn pause_protocol(ctx: Context<OperatorOnly>) -> Result<()> {
        let config = &mut ctx.accounts.protocol_config;
        require!(!config.is_paused, StakeFlowError::AlreadyPaused);
        config.is_paused = true;

        emit!(ProtocolPaused {
            paused_by: ctx.accounts.authority.key(),
        });

        Ok(())
    }

    /// Only admin can unpause. Operator can pause but not unpause for safety.
    pub fn unpause_protocol(ctx: Context<AdminOnly>) -> Result<()> {
        let config = &mut ctx.accounts.protocol_config;
        require!(config.is_paused, StakeFlowError::NotPaused);
        config.is_paused = false;

        emit!(ProtocolUnpaused {
            unpaused_by: ctx.accounts.admin.key(),
        });

        Ok(())
    }

    /// Liquidity manager withdraws from reserve vault for external yield farming.
    pub fn withdraw_reserves(ctx: Context<LiquidityManagerAction>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.protocol_config;

        require!(amount > 0, StakeFlowError::ZeroAmount);
        require!(
            ctx.accounts.reserve_vault.amount >= amount,
            StakeFlowError::InsufficientVaultBalance
        );

        let config_seeds = &[b"protocol_config".as_ref(), &[config.bump]];
        let signer_seeds = &[&config_seeds[..]];

        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.reserve_vault.to_account_info(),
                    mint: ctx.accounts.stake_token_mint.to_account_info(),
                    to: ctx.accounts.manager_token_account.to_account_info(),
                    authority: ctx.accounts.protocol_config.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
            ctx.accounts.stake_token_mint.decimals,
        )?;

        emit!(ReservesWithdrawn {
            manager: ctx.accounts.liquidity_manager.key(),
            amount,
        });

        Ok(())
    }

    /// Liquidity manager deposits yield back into the reserve vault.
    pub fn deposit_yield(ctx: Context<LiquidityManagerAction>, amount: u64) -> Result<()> {
        require!(amount > 0, StakeFlowError::ZeroAmount);

        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.manager_token_account.to_account_info(),
                    mint: ctx.accounts.stake_token_mint.to_account_info(),
                    to: ctx.accounts.reserve_vault.to_account_info(),
                    authority: ctx.accounts.liquidity_manager.to_account_info(),
                },
            ),
            amount,
            ctx.accounts.stake_token_mint.decimals,
        )?;

        emit!(YieldDeposited {
            manager: ctx.accounts.liquidity_manager.key(),
            amount,
        });

        Ok(())
    }

    /// Liquidity manager rebalances between stake vault and reserve vault.
    /// Can move tokens in either direction to optimize yield.
    pub fn rebalance_pools(
        ctx: Context<RebalancePools>,
        amount: u64,
        from_stake_to_reserve: bool,
    ) -> Result<()> {
        let config = &ctx.accounts.protocol_config;

        require!(amount > 0, StakeFlowError::ZeroAmount);

        // SECURITY: Ensure source and destination vaults are different
        require!(
            ctx.accounts.stake_vault.key() != ctx.accounts.reserve_vault.key(),
            StakeFlowError::DuplicateAccount
        );

        let config_seeds = &[b"protocol_config".as_ref(), &[config.bump]];
        let signer_seeds = &[&config_seeds[..]];

        if from_stake_to_reserve {
            require!(
                ctx.accounts.stake_vault.amount >= amount,
                StakeFlowError::InsufficientVaultBalance
            );

            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.stake_vault.to_account_info(),
                        mint: ctx.accounts.stake_token_mint.to_account_info(),
                        to: ctx.accounts.reserve_vault.to_account_info(),
                        authority: ctx.accounts.protocol_config.to_account_info(),
                    },
                    signer_seeds,
                ),
                amount,
                ctx.accounts.stake_token_mint.decimals,
            )?;
        } else {
            require!(
                ctx.accounts.reserve_vault.amount >= amount,
                StakeFlowError::InsufficientVaultBalance
            );

            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.reserve_vault.to_account_info(),
                        mint: ctx.accounts.stake_token_mint.to_account_info(),
                        to: ctx.accounts.stake_vault.to_account_info(),
                        authority: ctx.accounts.protocol_config.to_account_info(),
                    },
                    signer_seeds,
                ),
                amount,
                ctx.accounts.stake_token_mint.decimals,
            )?;
        }

        emit!(PoolsRebalanced {
            manager: ctx.accounts.liquidity_manager.key(),
            amount,
            direction: if from_stake_to_reserve {
                "stake_to_reserve".to_string()
            } else {
                "reserve_to_stake".to_string()
            },
        });

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────
// ACCOUNT STRUCTS (Contexts)
// ─────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = ProtocolConfig::LEN,
        seeds = [b"protocol_config"],
        bump,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,

    /// The base token that users stake (Token X)
    pub stake_token_mint: InterfaceAccount<'info, Mint>,

    /// stX receipt token mint — controlled by protocol_config PDA
    #[account(
        init,
        payer = admin,
        seeds = [b"stx_mint"],
        bump,
        mint::decimals = stake_token_mint.decimals,
        mint::authority = protocol_config,
        mint::freeze_authority = protocol_config,
    )]
    pub stx_mint: InterfaceAccount<'info, Mint>,

    /// Vault holding staked tokens
    #[account(
        init,
        payer = admin,
        seeds = [b"stake_vault"],
        bump,
        token::mint = stake_token_mint,
        token::authority = protocol_config,
    )]
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    /// Vault holding reserve/yield tokens
    #[account(
        init,
        payer = admin,
        seeds = [b"reserve_vault"],
        bump,
        token::mint = stake_token_mint,
        token::authority = protocol_config,
    )]
    pub reserve_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct AdminOnly<'info> {
    #[account(
        mut,
        seeds = [b"protocol_config"],
        bump = protocol_config.bump,
        has_one = admin @ StakeFlowError::UnauthorizedAdmin,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,

    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct OperatorOnly<'info> {
    #[account(
        mut,
        seeds = [b"protocol_config"],
        bump = protocol_config.bump,
        // SECURITY: Operator OR admin can use operator functions
        constraint = protocol_config.operator == authority.key()
            || protocol_config.admin == authority.key()
            @ StakeFlowError::UnauthorizedOperator,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,

    pub authority: Signer<'info>,
}

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
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    /// User's token X account (source)
    #[account(
        mut,
        token::mint = stake_token_mint,
        token::authority = user,
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    /// User's stX account (destination for receipt tokens)
    #[account(
        mut,
        token::mint = stx_mint,
        token::authority = user,
    )]
    pub user_stx_account: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct StakeLocked<'info> {
    #[account(
        mut,
        seeds = [b"protocol_config"],
        bump = protocol_config.bump,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,

    pub stake_token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [b"stake_vault"],
        bump = protocol_config.stake_vault_bump,
        constraint = stake_vault.key() == protocol_config.stake_vault @ StakeFlowError::InvalidVault,
    )]
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    /// User's token X account (source)
    #[account(
        mut,
        token::mint = stake_token_mint,
        token::authority = user,
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    /// Per-user locked staking position
    // SECURITY: PDA includes user key to prevent cross-user access
    #[account(
        init,
        payer = user,
        space = UserStake::LEN,
        seeds = [b"user_stake", user.key().as_ref()],
        bump,
    )]
    pub user_stake: Account<'info, UserStake>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UnstakeLiquid<'info> {
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
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = stake_token_mint,
        token::authority = user,
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = stx_mint,
        token::authority = user,
    )]
    pub user_stx_account: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct UnstakeLocked<'info> {
    #[account(
        mut,
        seeds = [b"protocol_config"],
        bump = protocol_config.bump,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,

    pub stake_token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [b"stake_vault"],
        bump = protocol_config.stake_vault_bump,
        constraint = stake_vault.key() == protocol_config.stake_vault @ StakeFlowError::InvalidVault,
    )]
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = stake_token_mint,
        token::authority = user,
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        // SECURITY: close returns rent to user (trusted destination)
        close = user,
        seeds = [b"user_stake", user.key().as_ref()],
        bump = user_stake.bump,
        has_one = user @ StakeFlowError::UnauthorizedUser,
    )]
    pub user_stake: Account<'info, UserStake>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(
        mut,
        seeds = [b"protocol_config"],
        bump = protocol_config.bump,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,

    pub stake_token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [b"stake_vault"],
        bump = protocol_config.stake_vault_bump,
        constraint = stake_vault.key() == protocol_config.stake_vault @ StakeFlowError::InvalidVault,
    )]
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = stake_token_mint,
        token::authority = user,
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"user_stake", user.key().as_ref()],
        bump = user_stake.bump,
        has_one = user @ StakeFlowError::UnauthorizedUser,
    )]
    pub user_stake: Account<'info, UserStake>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct LiquidityManagerAction<'info> {
    #[account(
        seeds = [b"protocol_config"],
        bump = protocol_config.bump,
        // SECURITY: Only the designated liquidity manager can perform these actions
        constraint = protocol_config.liquidity_manager == liquidity_manager.key()
            @ StakeFlowError::UnauthorizedLiquidityManager,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,

    pub stake_token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [b"reserve_vault"],
        bump = protocol_config.reserve_vault_bump,
        constraint = reserve_vault.key() == protocol_config.reserve_vault @ StakeFlowError::InvalidVault,
    )]
    pub reserve_vault: InterfaceAccount<'info, TokenAccount>,

    /// Liquidity manager's own token account for yield farming
    #[account(
        mut,
        token::mint = stake_token_mint,
        token::authority = liquidity_manager,
    )]
    pub manager_token_account: InterfaceAccount<'info, TokenAccount>,

    pub liquidity_manager: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct RebalancePools<'info> {
    #[account(
        seeds = [b"protocol_config"],
        bump = protocol_config.bump,
        constraint = protocol_config.liquidity_manager == liquidity_manager.key()
            @ StakeFlowError::UnauthorizedLiquidityManager,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,

    pub stake_token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [b"stake_vault"],
        bump = protocol_config.stake_vault_bump,
        constraint = stake_vault.key() == protocol_config.stake_vault @ StakeFlowError::InvalidVault,
    )]
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"reserve_vault"],
        bump = protocol_config.reserve_vault_bump,
        constraint = reserve_vault.key() == protocol_config.reserve_vault @ StakeFlowError::InvalidVault,
    )]
    pub reserve_vault: InterfaceAccount<'info, TokenAccount>,

    pub liquidity_manager: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

// ─────────────────────────────────────────────────────────────
// STATE
// ─────────────────────────────────────────────────────────────

#[account]
pub struct ProtocolConfig {
    pub admin: Pubkey,                  // 32
    pub operator: Pubkey,               // 32
    pub liquidity_manager: Pubkey,      // 32
    pub stake_token_mint: Pubkey,       // 32
    pub stx_mint: Pubkey,               // 32
    pub stake_vault: Pubkey,            // 32
    pub reserve_vault: Pubkey,          // 32
    pub liquid_reward_rate_bps: u64,    // 8
    pub locked_reward_rate_bps: u64,    // 8
    pub total_staked: u64,              // 8
    pub total_locked: u64,              // 8
    pub total_rewards_distributed: u64, // 8
    pub is_paused: bool,                // 1
    pub last_update_timestamp: i64,     // 8
    pub bump: u8,                       // 1
    pub stake_vault_bump: u8,           // 1
    pub reserve_vault_bump: u8,         // 1
    pub stx_mint_bump: u8,              // 1
}

impl ProtocolConfig {
    // discriminator(8) + 7*32 + 5*8 + 1 + 8 + 4*1
    pub const LEN: usize = 8 + (7 * 32) + (5 * 8) + 1 + 8 + 4;
}

#[account]
pub struct UserStake {
    pub user: Pubkey,         // 32
    pub amount: u64,          // 8
    pub stake_timestamp: i64, // 8
    pub lockup_expiry: i64,   // 8
    pub reward_debt: u64,     // 8
    pub is_active: bool,      // 1
    pub bump: u8,             // 1
}

impl UserStake {
    // discriminator(8) + 32 + 8 + 8 + 8 + 8 + 1 + 1
    pub const LEN: usize = 8 + 32 + 8 + 8 + 8 + 8 + 1 + 1;
}

// ─────────────────────────────────────────────────────────────
// EVENTS
// ─────────────────────────────────────────────────────────────

#[event]
pub struct ProtocolInitialized {
    pub admin: Pubkey,
    pub liquid_rate: u64,
    pub locked_rate: u64,
}

#[event]
pub struct OperatorChanged {
    pub old_operator: Pubkey,
    pub new_operator: Pubkey,
}

#[event]
pub struct LiquidityManagerChanged {
    pub old_manager: Pubkey,
    pub new_manager: Pubkey,
}

#[event]
pub struct LiquidStaked {
    pub user: Pubkey,
    pub amount: u64,
    pub stx_minted: u64,
}

#[event]
pub struct LockedStaked {
    pub user: Pubkey,
    pub amount: u64,
    pub lockup_expiry: i64,
}

#[event]
pub struct LiquidUnstaked {
    pub user: Pubkey,
    pub stx_burned: u64,
    pub tokens_returned: u64,
}

#[event]
pub struct LockedUnstaked {
    pub user: Pubkey,
    pub amount: u64,
    pub rewards: u64,
}

#[event]
pub struct RewardsClaimed {
    pub user: Pubkey,
    pub amount: u64,
}

#[event]
pub struct RewardRatesUpdated {
    pub liquid_rate: u64,
    pub locked_rate: u64,
}

#[event]
pub struct ProtocolPaused {
    pub paused_by: Pubkey,
}

#[event]
pub struct ProtocolUnpaused {
    pub unpaused_by: Pubkey,
}

#[event]
pub struct ReservesWithdrawn {
    pub manager: Pubkey,
    pub amount: u64,
}

#[event]
pub struct YieldDeposited {
    pub manager: Pubkey,
    pub amount: u64,
}

#[event]
pub struct PoolsRebalanced {
    pub manager: Pubkey,
    pub amount: u64,
    pub direction: String,
}

// ─────────────────────────────────────────────────────────────
// ERRORS
// ─────────────────────────────────────────────────────────────

#[error_code]
pub enum StakeFlowError {
    #[msg("Only the admin can perform this action")]
    UnauthorizedAdmin,

    #[msg("Only the operator or admin can perform this action")]
    UnauthorizedOperator,

    #[msg("Only the liquidity manager can perform this action")]
    UnauthorizedLiquidityManager,

    #[msg("Only the stake owner can perform this action")]
    UnauthorizedUser,

    #[msg("Protocol is currently paused")]
    ProtocolPaused,

    #[msg("Protocol is not paused")]
    NotPaused,

    #[msg("Protocol is already paused")]
    AlreadyPaused,

    #[msg("Amount must be greater than zero")]
    ZeroAmount,

    #[msg("Amount too small — would result in zero output")]
    AmountTooSmall,

    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,

    #[msg("Reward rate exceeds maximum allowed")]
    RewardRateTooHigh,

    #[msg("Locked reward rate must be >= liquid reward rate")]
    LockedRateMustExceedLiquid,

    #[msg("Lockup period has not expired yet")]
    LockupNotExpired,

    #[msg("Stake position is not active")]
    StakeNotActive,

    #[msg("No rewards available to claim")]
    NoRewardsToClaim,

    #[msg("Insufficient balance in vault")]
    InsufficientVaultBalance,

    #[msg("No liquidity available")]
    NoLiquidity,

    #[msg("Invalid mint account")]
    InvalidMint,

    #[msg("Invalid vault account")]
    InvalidVault,

    #[msg("Source and destination accounts must be different")]
    DuplicateAccount,
}
