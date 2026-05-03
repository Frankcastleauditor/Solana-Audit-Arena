use anchor_lang_v2::prelude::*;
use anchor_lang_v2::programs::Token;
use anchor_spl_v2::{
    mint::Mint,
    token::{self, cpi as token_cpi, TokenAccount},
};
use pinocchio::cpi::{Seed, Signer as CpiSigner};
use pinocchio_system::instructions::Transfer;

use crate::constants::{
    BPS_DENOMINATOR, FRANKSOL_MINT_SEED, POOL_SEED, USER_POSITION_SEED, VAULT_SEED,
};
use crate::error::StakeError;
use crate::state::{Pool, UserPosition};
use crate::utils::{checked_sub_u64, franksol_to_sol};

#[derive(Accounts)]
pub struct Unstake {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, seeds = [POOL_SEED], bump = pool.bump)]
    pub pool: Account<Pool>,
    /// CHECK: Vault PDA holding SOL.
    #[account(mut, seeds = [VAULT_SEED], bump = pool.vault_bump)]
    pub vault: UncheckedAccount,
    /// CHECK: Constrained to pool.fee_recipients[0].pubkey.
    #[account(mut, address = pool.fee_recipients[0].pubkey @ StakeError::FeeRecipientMismatch)]
    pub fee_recipient_0: UncheckedAccount,
    /// CHECK: Constrained to pool.fee_recipients[1].pubkey.
    #[account(mut, address = pool.fee_recipients[1].pubkey @ StakeError::FeeRecipientMismatch)]
    pub fee_recipient_1: UncheckedAccount,
    /// CHECK: Constrained to pool.fee_recipients[2].pubkey.
    #[account(mut, address = pool.fee_recipients[1].pubkey @ StakeError::FeeRecipientMismatch)]
    pub fee_recipient_2: UncheckedAccount,
    #[account(mut, seeds = [FRANKSOL_MINT_SEED], bump)]
    pub franksol_mint: Account<Mint>,
    #[account(
        mut,
        token::mint = franksol_mint,
        token::authority = user
    )]
    pub user_franksol_ata: Account<TokenAccount>,
    #[account(
        mut,
        seeds = [USER_POSITION_SEED, user.address().as_ref()],
        bump = user_position.bump
    )]
    pub user_position: Account<UserPosition>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

pub fn handler(ctx: &mut Context<Unstake>, franksol_in: u64, min_sol_out: u64) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let user_position = &mut ctx.accounts.user_position;
    require!(franksol_in > 0, StakeError::InvalidAmount);
    require_keys_eq!(
        user_position.owner,
        *ctx.accounts.user.address(),
        StakeError::Unauthorized
    );
    require!(!user_position.is_blacklisted.get(), StakeError::UserBlacklisted);

    let sol_out = franksol_to_sol(franksol_in, pool.total_sol.get(), pool.franksol_supply.get())?;
    let fee_bps = [
        pool.fee_recipients[0].fee_bps.get(),
        pool.fee_recipients[1].fee_bps.get(),
        pool.fee_recipients[2].fee_bps.get(),
    ];
    let mut recipient_fees: [u64; 3] = [0; 3];
    let mut total_fee: u64 = 0;
    for (idx, bps) in fee_bps.iter().enumerate() {
        if *bps == 0 {
            continue;
        }
        let fee = (sol_out as u128)
            .checked_mul((*bps).into())
            .ok_or(StakeError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(StakeError::MathOverflow)?;
        let fee = u64::try_from(fee).map_err(|_| StakeError::MathOverflow)?;
        recipient_fees[idx] = fee;
        total_fee = total_fee.checked_add(fee).ok_or(StakeError::MathOverflow)?;
    }
    let user_sol_out = checked_sub_u64(sol_out, total_fee)?;
    require!(sol_out >= min_sol_out, StakeError::SlippageExceeded);

    let liquid_sol = checked_sub_u64(pool.total_sol.get(), pool.deployed_sol.get())?;
    require!(sol_out <= liquid_sol, StakeError::InsufficientVaultBalance);

    let burn_ctx = CpiContext::new(
        ctx.accounts.token_program.address(),
        token_cpi::accounts::Burn {
            account: ctx.accounts.user_franksol_ata.cpi_handle_mut(),
            mint: ctx.accounts.franksol_mint.cpi_handle_mut(),
            authority: ctx.accounts.user.cpi_handle(),
        },
    );
    token_cpi::burn(burn_ctx, franksol_in)?;

    let vault_bump_bytes = [pool.vault_bump];
    let vault_seed_arr = [Seed::from(VAULT_SEED), Seed::from(&vault_bump_bytes[..])];
    let vault_signer = CpiSigner::from(&vault_seed_arr);
    Transfer {
        from: ctx.accounts.vault.account(),
        to: ctx.accounts.user.account(),
        lamports: user_sol_out,
    }
    .invoke_signed(&[vault_signer])?;
    let fee_recipients = [
        &ctx.accounts.fee_recipient_0,
        &ctx.accounts.fee_recipient_1,
        &ctx.accounts.fee_recipient_2,
    ];
    for (idx, recipient_account) in fee_recipients.iter().enumerate() {
        let recipient_fee = recipient_fees[idx];
        if recipient_fee == 0 {
            continue;
        }
        let signer = CpiSigner::from(&vault_seed_arr);
        Transfer {
            from: ctx.accounts.vault.account(),
            to: recipient_account.account(),
            lamports: recipient_fee,
        }
        .invoke_signed(&[signer])?;
    }

    pool.total_sol = PodU64::from(checked_sub_u64(pool.total_sol.get(), sol_out)?);
    pool.franksol_supply = PodU64::from(checked_sub_u64(
        pool.franksol_supply.get(),
        franksol_in,
    )?);

    user_position.franksol_balance =
        PodU64::from(user_position.franksol_balance.get().saturating_sub(franksol_in));
    Ok(())
}
