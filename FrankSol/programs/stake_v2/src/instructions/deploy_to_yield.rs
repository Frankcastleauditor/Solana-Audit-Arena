use anchor_lang_v2::prelude::*;
use yield_generator::YieldState;

use crate::constants::{POOL_SEED, VAULT_SEED};
use crate::error::StakeError;
use crate::state::Pool;
use crate::utils::{checked_add_u64, checked_sub_u64};

#[derive(Accounts)]
pub struct DeployToYield {
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
    /// CHECK: Per-user position PDA created by yield_generator::deposit CPI.
    #[account(
        mut,
        seeds = [b"user_position", fund_manager.address().as_ref()],
        bump,
        seeds::program = yield_generator_program.address()
    )]
    pub yield_position: UncheckedAccount,
    /// CHECK: Yield strategy vault controlled by external program.
    #[account(
        mut,
        seeds = [b"yield_vault"],
        bump = yield_state.vault_bump,
        seeds::program = yield_generator_program.address()
    )]
    pub yield_vault: UncheckedAccount,
    #[account(address = yield_generator::id())]
    pub yield_generator_program: UncheckedAccount,
    pub system_program: Program<System>,
}

pub fn handler(ctx: &mut Context<DeployToYield>, amount: u64) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    require!(amount > 0, StakeError::InvalidAmount);
    let liquid_sol = checked_sub_u64(pool.total_sol.get(), pool.deployed_sol.get())?;
    require!(amount <= liquid_sol, StakeError::InsufficientVaultBalance);

    let vault_bump = [pool.vault_bump];
    let vault_seed_bytes: &[&[u8]] = &[VAULT_SEED, &vault_bump];
    let signer_seeds: &[&[&[u8]]] = &[vault_seed_bytes];
    let cpi_accounts = yield_generator::cpi::accounts::Deposit {
        operator: ctx.accounts.fund_manager.cpi_handle(),
        state: ctx.accounts.yield_state.cpi_handle_mut(),
        position: ctx.accounts.yield_position.cpi_handle_mut(),
        source_vault: ctx.accounts.vault.cpi_handle(),
        yield_vault: ctx.accounts.yield_vault.cpi_handle_mut(),
        system_program: ctx.accounts.system_program.cpi_handle(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.yield_generator_program.address(), cpi_accounts)
        .with_signer(&signer_seeds);
    yield_generator::cpi::deposit(cpi_ctx, amount)?;

    pool.deployed_sol = PodU64::from(checked_add_u64(pool.deployed_sol.get(), amount)?);
    Ok(())
}
