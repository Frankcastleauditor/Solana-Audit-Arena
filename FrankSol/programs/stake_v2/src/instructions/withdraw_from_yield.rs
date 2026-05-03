use anchor_lang_v2::prelude::*;
use yield_generator::{UserPosition, YieldState};

use crate::constants::{POOL_SEED, VAULT_SEED};
use crate::error::StakeError;
use crate::state::Pool;
use crate::utils::{checked_add_u64, checked_sub_u64};

#[derive(Accounts)]
pub struct WithdrawFromYield {
    pub fund_manager: Signer,
    #[account(mut, seeds = [POOL_SEED], bump = pool.bump)]
    pub pool: Account<Pool>,
    /// CHECK: Vault PDA holding SOL.
    #[account(mut, seeds = [VAULT_SEED], bump = pool.vault_bump)]
    pub vault: UncheckedAccount,
    #[account(
        mut,
        seeds = [b"yield_state"],
        bump = yield_state.state_bump,
        seeds::program = yield_generator_program.address()
    )]
    pub yield_state: Account<YieldState>,
    #[account(mut, constraint = yield_position.owner == *fund_manager.address() @ StakeError::Unauthorized)]
    pub yield_position: Account<UserPosition>,
    /// CHECK: External strategy vault account where deposited SOL sits.
    #[account(
        mut,
        seeds = [b"yield_vault"],
        bump = yield_state.vault_bump,
        seeds::program = yield_generator_program.address()
    )]
    pub yield_vault: UncheckedAccount,
    /// CHECK: External yield program invoked via CPI.
    pub yield_generator_program: UncheckedAccount,
    pub system_program: Program<System>,
}

pub fn handler(
    ctx: &mut Context<WithdrawFromYield>,
    principal_returned: u64,
    yield_amount: u64,
) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    require_keys_eq!(
        *ctx.accounts.fund_manager.address(),
        pool.fund_manager,
        StakeError::Unauthorized
    );
    require!(principal_returned > 0, StakeError::InvalidAmount);
    require!(
        principal_returned <= pool.deployed_sol.get(),
        StakeError::InsufficientDeployed
    );
    let vault_before = ctx.accounts.vault.lamports();

    let cpi_accounts = yield_generator::cpi::accounts::Withdraw {
        operator: ctx.accounts.fund_manager.cpi_handle(),
        state: ctx.accounts.yield_state.cpi_handle_mut(),
        position: ctx.accounts.yield_position.cpi_handle_mut(),
        yield_vault: ctx.accounts.yield_vault.cpi_handle_mut(),
        destination_vault: ctx.accounts.vault.cpi_handle_mut(),
        system_program: ctx.accounts.system_program.cpi_handle(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.yield_generator_program.address(), cpi_accounts);
    yield_generator::cpi::withdraw(cpi_ctx, principal_returned, yield_amount)?;

    let vault_after = ctx.accounts.vault.lamports();
    let total_return = checked_sub_u64(vault_after, vault_before)?;
    let pnl_i128 = (total_return as i128) - (principal_returned as i128);

    pool.deployed_sol = PodU64::from(checked_sub_u64(
        pool.deployed_sol.get(),
        principal_returned,
    )?);
    if pnl_i128 >= 0 {
        let gain = u64::try_from(pnl_i128).map_err(|_| StakeError::MathOverflow)?;
        pool.total_sol = PodU64::from(checked_sub_u64(pool.total_sol.get(), gain)?);
    } else {
        let loss = u64::try_from(-pnl_i128).map_err(|_| StakeError::MathOverflow)?;
        pool.total_sol = PodU64::from(checked_add_u64(pool.total_sol.get(), loss)?);
    }
    Ok(())
}
