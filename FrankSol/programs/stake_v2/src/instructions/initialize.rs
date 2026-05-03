use anchor_lang_v2::prelude::*;
use anchor_lang_v2::programs::Token;
use anchor_spl_v2::mint::{self, Mint};
use pinocchio::cpi::{Seed, Signer as CpiSigner};
use pinocchio_system::instructions::CreateAccount;

use crate::constants::MAX_TOTAL_FEE_BPS;
use crate::constants::{FRANKSOL_MINT_SEED, MINT_AUTH_SEED, POOL_SEED, VAULT_SEED};
use crate::error::StakeError;
use crate::state::{FeeRecipient, Pool};

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub admin: Signer,
    /// CHECK: Authority pubkey managed by admin.
    pub fund_manager: UncheckedAccount,
    /// CHECK: Treasury pubkey configured by admin.
    pub treasury: UncheckedAccount,
    #[account(
        init,
        payer = admin,
        space = 8 + core::mem::size_of::<Pool>(),
        seeds = [POOL_SEED],
        bump
    )]
    pub pool: Account<Pool>,
    /// CHECK: PDA that acts as mint/freeze authority; no data, never owns lamports.
    #[account(
        seeds = [MINT_AUTH_SEED],
        bump
    )]
    pub mint_authority: UncheckedAccount,
    #[account(
        init,
        payer = admin,
        mint::decimals = 9,
        mint::authority = mint_authority,
        mint::freeze_authority = mint_authority,
        seeds = [FRANKSOL_MINT_SEED],
        bump
    )]
    pub franksol_mint: Account<Mint>,
    /// CHECK: Created as a PDA system account with zero data.
    #[account(mut, seeds = [VAULT_SEED], bump)]
    pub vault: UncheckedAccount,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

pub fn handler(ctx: &mut Context<Initialize>) -> Result<()> {
    let pool = &mut ctx.accounts.pool;

    if ctx.accounts.vault.account().data_len() == 0 {
        let vault_bump_bytes = [ctx.bumps.vault];
        let seeds = [Seed::from(VAULT_SEED), Seed::from(&vault_bump_bytes[..])];
        let signer = CpiSigner::from(&seeds);
        CreateAccount {
            from: ctx.accounts.admin.account(),
            to: ctx.accounts.vault.account(),
            lamports: anchor_lang_v2::cpi::rent_exempt_lamports(0)?,
            space: 0,
            owner: ctx.program_id,
        }
        .invoke_signed(&[signer])?;
    } else {
        return Err(StakeError::AlreadyInitialized.into());
    }

    pool.admin = *ctx.accounts.admin.address();
    pool.fund_manager = *ctx.accounts.fund_manager.address();
    pool.fee_recipients = [
        FeeRecipient {
            pubkey: *ctx.accounts.treasury.address(),
            fee_bps: PodU16::from(MAX_TOTAL_FEE_BPS),
        },
        FeeRecipient::default(),
        FeeRecipient::default(),
    ];
    pool.franksol_mint = *ctx.accounts.franksol_mint.address();
    pool.total_sol = PodU64::from(0);
    pool.deployed_sol = PodU64::from(0);
    pool.franksol_supply = PodU64::from(0);
    pool.bump = ctx.bumps.pool;
    pool.vault_bump = ctx.bumps.vault;
    pool.mint_auth_bump = ctx.bumps.mint_authority;

    Ok(())
}
