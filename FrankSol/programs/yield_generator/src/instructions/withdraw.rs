use anchor_lang_v2::prelude::*;

use crate::error::YieldError;
use crate::state::{UserPosition, YieldState};
use crate::{BPS_DENOMINATOR, POSITION_SEED, SECONDS_PER_YEAR, STATE_SEED, VAULT_SEED};

#[derive(Accounts)]
pub struct Withdraw {
    #[account(mut)]
    pub operator: Signer,
    #[account(mut, seeds = [STATE_SEED], bump = state.state_bump)]
    pub state: Account<YieldState>,
    #[account(
        mut,
        close = operator,
        seeds = [POSITION_SEED, operator.address().as_ref()],
        bump = position.bump
    )]
    pub position: Account<UserPosition>,
    /// CHECK: Yield vault PDA that holds strategy SOL and is validated by seeds/bump.
    #[account(mut, seeds = [VAULT_SEED], bump = state.vault_bump)]
    pub yield_vault: UncheckedAccount,
    /// CHECK: Destination vault in caller program.
    #[account(mut)]
    pub destination_vault: UncheckedAccount,
    pub system_program: Program<System>,
}

pub fn handler(
    ctx: &mut Context<Withdraw>,
    principal_returned: u64,
    _yield_amount: u64,
) -> Result<()> {
    require!(principal_returned > 0, YieldError::InvalidAmount);

    let now = Clock::get()?.unix_timestamp;
    let state = &mut ctx.accounts.state;
    let position = &mut ctx.accounts.position;
    require_keys_eq!(
        position.owner,
        *ctx.accounts.operator.address(),
        YieldError::Unauthorized
    );
    require!(
        principal_returned == position.principal.get(),
        YieldError::PartialWithdrawNotAllowed
    );

    accrue_rewards(position, state.apy_bps.get(), now)?;

    let reward_for_withdrawal = position.accrued_reward.get();
    let total_out = if state.yield_direction_positive.get() {
        principal_returned
            .checked_add(reward_for_withdrawal)
            .ok_or(YieldError::MathOverflow)?
    } else {
        principal_returned.saturating_sub(reward_for_withdrawal)
    };

    let mut vault = *ctx.accounts.yield_vault.account();
    let mut destination = *ctx.accounts.destination_vault.account();
    require!(
        vault.lamports() >= total_out,
        YieldError::InsufficientVaultBalance
    );
    vault.set_lamports(
        vault
            .lamports()
            .checked_sub(total_out)
            .ok_or(YieldError::MathOverflow)?,
    );
    destination.set_lamports(
        destination
            .lamports()
            .checked_add(total_out)
            .ok_or(YieldError::MathOverflow)?,
    );

    position.principal = PodU64::from(
        position
            .principal
            .get()
            .checked_sub(principal_returned)
            .ok_or(YieldError::MathOverflow)?,
    );
    position.accrued_reward = PodU64::from(
        position
            .accrued_reward
            .get()
            .checked_sub(reward_for_withdrawal)
            .ok_or(YieldError::MathOverflow)?,
    );

    state.total_principal = PodU64::from(
        state
            .total_principal
            .get()
            .checked_sub(principal_returned)
            .ok_or(YieldError::MathOverflow)?,
    );
    let yield_paid = if state.yield_direction_positive.get() {
        reward_for_withdrawal
    } else {
        0
    };
    state.total_yield_paid = PodU64::from(
        state
            .total_yield_paid
            .get()
            .checked_add(yield_paid)
            .ok_or(YieldError::MathOverflow)?,
    );
    Ok(())
}

fn accrue_rewards(position: &mut Account<UserPosition>, apy_bps: u16, now: i64) -> Result<()> {
    let last_update = position.last_update_ts.get();
    require!(now >= last_update, YieldError::InvalidTime);
    let elapsed = now
        .checked_sub(last_update)
        .ok_or(YieldError::MathOverflow)?;

    if elapsed > 0 && position.principal.get() > 0 {
        let reward_u128 = (position.principal.get() as u128)
            .checked_mul(apy_bps as u128)
            .ok_or(YieldError::MathOverflow)?
            .checked_mul(elapsed as u128)
            .ok_or(YieldError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(YieldError::MathOverflow)?
            .checked_div(SECONDS_PER_YEAR as u128)
            .ok_or(YieldError::MathOverflow)?;
        let reward = u64::try_from(reward_u128).map_err(|_| YieldError::MathOverflow)?;
        position.accrued_reward = PodU64::from(
            position
                .accrued_reward
                .get()
                .checked_add(reward)
                .ok_or(YieldError::MathOverflow)?,
        );
    }

    position.last_update_ts = PodI64::from(now);
    Ok(())
}
