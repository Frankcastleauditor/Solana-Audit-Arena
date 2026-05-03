use anchor_lang_v2::prelude::*;
use anchor_lang_v2::programs::Token;
use anchor_spl_v2::{
    mint::Mint,
    token::{self, cpi as token_cpi, TokenAccount},
};
use pinocchio_system::instructions::Transfer;

use crate::constants::{
    FRANKSOL_MINT_SEED, MINT_AUTH_SEED, POOL_SEED, USER_POSITION_SEED, VAULT_SEED,
};
use crate::error::StakeError;
use crate::state::{Pool, UserPosition};
use crate::utils::{checked_add_u64, sol_to_franksol};

#[derive(Accounts)]
pub struct Stake {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, seeds = [POOL_SEED], bump = pool.bump)]
    pub pool: Account<Pool>,
    /// CHECK: Vault PDA holding SOL.
    #[account(mut, seeds = [VAULT_SEED], bump = pool.vault_bump)]
    pub vault: UncheckedAccount,
    #[account(mut, seeds = [FRANKSOL_MINT_SEED], bump)]
    pub franksol_mint: Account<Mint>,
    /// CHECK: PDA signing mint CPIs.
    #[account(seeds = [MINT_AUTH_SEED], bump = pool.mint_auth_bump)]
    pub mint_authority: UncheckedAccount,
    #[account(
        mut,
        token::mint = franksol_mint,
    )]
    pub user_franksol_ata: Account<TokenAccount>,
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + core::mem::size_of::<UserPosition>(),
        seeds = [USER_POSITION_SEED, user.address().as_ref()],
        bump
    )]
    pub user_position: Account<UserPosition>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

pub fn handler(ctx: &mut Context<Stake>, amount_sol: u64, min_franksol_out: u64) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let user_position = &mut ctx.accounts.user_position;
    require!(amount_sol > 0, StakeError::InvalidAmount);
    if user_position.owner == Address::default() {
        user_position.owner = *ctx.accounts.user.address();
        user_position.franksol_balance = PodU64::from(0);
        user_position.sol_deposited = PodU64::from(0);
        user_position.is_blacklisted = PodBool::from(false);
        user_position.bump = ctx.bumps.user_position;
    } else {
        require_keys_eq!(
            user_position.owner,
            *ctx.accounts.user.address(),
            StakeError::Unauthorized
        );
    }
    require!(!user_position.is_blacklisted.get(), StakeError::UserBlacklisted);

    let franksol_out =
        sol_to_franksol(amount_sol, pool.total_sol.get(), pool.franksol_supply.get())?;
    require!(
        franksol_out >= min_franksol_out,
        StakeError::SlippageExceeded
    );

    Transfer {
        from: ctx.accounts.user.account(),
        to: ctx.accounts.vault.account(),
        lamports: amount_sol,
    }
    .invoke()?;

    let bump_bytes = [pool.mint_auth_bump];
    let mint_seed_bytes: &[&[u8]] = &[MINT_AUTH_SEED, &bump_bytes];
    let signer_seeds: &[&[&[u8]]] = &[mint_seed_bytes];
    let mint_ctx = CpiContext::new(
        ctx.accounts.token_program.address(),
        token_cpi::accounts::MintTo {
            mint: ctx.accounts.franksol_mint.cpi_handle_mut(),
            to: ctx.accounts.user_franksol_ata.cpi_handle_mut(),
            authority: ctx.accounts.mint_authority.cpi_handle(),
        },
    )
    .with_signer(&signer_seeds);
    token_cpi::mint_to(mint_ctx, franksol_out)?;

    emit!(crate::StakeEvent {
        user: *ctx.accounts.user.address(),
        sol_deposited: amount_sol,
        franksol_minted: franksol_out,
        pool_total_sol: pool.total_sol.get(),
        franksol_supply: pool.franksol_supply.get(),
    });

    pool.total_sol = PodU64::from(checked_add_u64(pool.total_sol.get(), amount_sol)?);
    pool.franksol_supply =
        PodU64::from(checked_add_u64(pool.franksol_supply.get(), franksol_out)?);

    user_position.franksol_balance = PodU64::from(checked_add_u64(
        user_position.franksol_balance.get(),
        franksol_out,
    )?);
    user_position.sol_deposited =
        PodU64::from(checked_add_u64(user_position.sol_deposited.get(), amount_sol)?);
    Ok(())
}
