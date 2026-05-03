use anchor_lang_v2::prelude::*;
use pinocchio::cpi::{Seed, Signer as CpiSigner};
use pinocchio_system::instructions::CreateAccount;

use crate::state::YieldState;
use crate::{APY_BPS, STATE_SEED, VAULT_SEED};

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        space = 8 + core::mem::size_of::<YieldState>(),
        seeds = [STATE_SEED],
        bump
    )]
    pub state: Account<YieldState>,
    /// CHECK: System vault PDA created during initialize.
    #[account(mut, seeds = [VAULT_SEED], bump)]
    pub vault: UncheckedAccount,
    pub system_program: Program<System>,
}

pub fn handler(ctx: &mut Context<Initialize>) -> Result<()> {
    if ctx.accounts.vault.account().data_len() == 0 {
        let bump_bytes = [ctx.bumps.vault];
        let seeds = [Seed::from(VAULT_SEED), Seed::from(&bump_bytes[..])];
        let signer = CpiSigner::from(&seeds);
        CreateAccount {
            from: ctx.accounts.payer.account(),
            to: ctx.accounts.vault.account(),
            lamports: anchor_lang_v2::cpi::rent_exempt_lamports(0)?,
            space: 0,
            owner: ctx.program_id,
        }
        .invoke_signed(&[signer])?;
    }

    let state = &mut ctx.accounts.state;
    state.authority = *ctx.accounts.payer.address();
    state.apy_bps = PodU16::from(APY_BPS);
    state.total_principal = PodU64::from(0);
    state.total_yield_paid = PodU64::from(0);
    state.last_config_update_ts = PodI64::from(Clock::get()?.unix_timestamp);
    state.state_bump = ctx.bumps.state;
    state.vault_bump = ctx.bumps.vault;
    state.yield_direction_positive = PodBool::from(true);

    Ok(())
}
