use anchor_lang_v2::prelude::*;
use pinocchio_system::instructions::Transfer;

use crate::error::YieldError;
use crate::state::{UserPosition, YieldState};
use crate::{POSITION_SEED, STATE_SEED, VAULT_SEED};

#[derive(Accounts)]
pub struct Deposit {
    #[account(mut)]
    pub operator: Signer,
    #[account(mut, seeds = [STATE_SEED], bump = state.state_bump)]
    pub state: Account<YieldState>,
    #[account(
        init,
        payer = operator,
        space = 8 + core::mem::size_of::<UserPosition>(),
        seeds = [POSITION_SEED, operator.address().as_ref()],
        bump
    )]
    pub position: Account<UserPosition>,
    /// CHECK: Source vault from caller program; must sign this CPI.
    pub source_vault: Signer,
    /// CHECK: Yield vault PDA receiving deposits.
    #[account(mut, seeds = [VAULT_SEED], bump = state.vault_bump)]
    pub yield_vault: UncheckedAccount,
    pub system_program: Program<System>,
}

pub fn handler(ctx: &mut Context<Deposit>, amount: u64) -> Result<()> {
    require!(amount > 0, YieldError::InvalidAmount);

    let now = Clock::get()?.unix_timestamp;
    let position = &mut ctx.accounts.position;
    position.owner = *ctx.accounts.operator.address();
    position.bump = ctx.bumps.position;
    position.last_update_ts = PodI64::from(now);
    position.principal = PodU64::from(amount);
    position.accrued_reward = PodU64::from(0);

    Transfer {
        from: ctx.accounts.source_vault.account(),
        to: ctx.accounts.yield_vault.account(),
        lamports: amount,
    }
    .invoke()?;

    let next_total = ctx
        .accounts
        .state
        .total_principal
        .get()
        .checked_add(amount)
        .ok_or(YieldError::MathOverflow)?;
    ctx.accounts.state.total_principal = PodU64::from(next_total);
    Ok(())
}
