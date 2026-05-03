use anchor_lang_v2::prelude::*;
use anchor_lang_v2::programs::Token;
use anchor_spl_v2::{
    mint::Mint,
    token::{self, cpi as token_cpi, TokenAccount},
};

use crate::constants::{
    FRANKSOL_MINT_SEED, MAX_TOTAL_FEE_BPS, MINT_AUTH_SEED, POOL_SEED, USER_POSITION_SEED,
};
use crate::error::StakeError;
use crate::state::{FeeRecipient, FeeRecipientArg, Pool, UserPosition};

#[derive(Accounts)]
pub struct SetAdmin {
    pub admin: Signer,
    #[account(mut, seeds = [POOL_SEED], bump = pool.bump)]
    pub pool: Account<Pool>,
}

#[derive(Accounts)]
pub struct SetFundManager {
    pub admin: Signer,
    #[account(mut, seeds = [POOL_SEED], bump = pool.bump)]
    pub pool: Account<Pool>,
}

#[derive(Accounts)]
pub struct SetFeeRecipients {
    pub admin: Signer,
    #[account(mut, seeds = [POOL_SEED], bump = pool.bump)]
    pub pool: Account<Pool>,
}

#[derive(Accounts)]
pub struct SetUserBlacklist {
    pub admin: Signer,
    #[account(mut, seeds = [POOL_SEED], bump = pool.bump)]
    pub pool: Account<Pool>,
    /// CHECK: User whose position is being updated.
    pub user: UncheckedAccount,
    #[account(
        mut,
        seeds = [USER_POSITION_SEED, user.address().as_ref()],
        bump = user_position.bump
    )]
    pub user_position: Account<UserPosition>,
    #[account(mut, seeds = [FRANKSOL_MINT_SEED], bump)]
    pub franksol_mint: Account<Mint>,
    /// CHECK: PDA with freeze authority over the frankSOL mint.
    #[account(seeds = [MINT_AUTH_SEED], bump = pool.mint_auth_bump)]
    pub mint_authority: UncheckedAccount,
    #[account(
        mut,
        token::mint = franksol_mint,
        token::authority = user,
    )]
    pub user_franksol_ata: Account<TokenAccount>,
    pub token_program: Program<Token>,
}

pub fn set_admin(ctx: &mut Context<SetAdmin>, new_admin: Address) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    require_keys_eq!(
        *ctx.accounts.admin.address(),
        pool.admin,
        StakeError::Unauthorized
    );
    pool.admin = new_admin;
    Ok(())
}

pub fn set_fund_manager(ctx: &mut Context<SetFundManager>, new_fm: Address) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    require_keys_eq!(
        *ctx.accounts.admin.address(),
        pool.admin,
        StakeError::Unauthorized
    );
    pool.fund_manager = new_fm;
    Ok(())
}

pub fn set_fee_recipients(
    ctx: &mut Context<SetFeeRecipients>,
    slots: [FeeRecipientArg; 3],
) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    require_keys_eq!(
        *ctx.accounts.admin.address(),
        pool.admin,
        StakeError::Unauthorized
    );

    let total_bps = slots.iter().try_fold(0u16, |acc, slot| {
        acc.checked_add(slot.fee_bps)
            .ok_or(StakeError::MathOverflow)
    })?;
    require!(total_bps <= MAX_TOTAL_FEE_BPS, StakeError::FeesExceedCap);

    pool.fee_recipients = slots.map(|slot| FeeRecipient {
        pubkey: slot.pubkey,
        fee_bps: PodU16::from(slot.fee_bps),
    });
    Ok(())
}

pub fn set_user_blacklist(
    ctx: &mut Context<SetUserBlacklist>,
    is_blacklisted: bool,
) -> Result<()> {
    let pool = &ctx.accounts.pool;
    require_keys_eq!(
        *ctx.accounts.admin.address(),
        pool.admin,
        StakeError::Unauthorized
    );
    require_keys_eq!(
        ctx.accounts.user_position.owner,
        *ctx.accounts.user.address(),
        StakeError::Unauthorized
    );

    ctx.accounts.user_position.is_blacklisted = PodBool::from(is_blacklisted);

    let bump = [pool.mint_auth_bump];
    let signer_seeds: [&[u8]; 2] = [MINT_AUTH_SEED, &bump];

    if is_blacklisted {
        let cpi_accounts = token_cpi::accounts::FreezeAccount {
            account: ctx.accounts.user_franksol_ata.cpi_handle_mut(),
            mint: ctx.accounts.franksol_mint.cpi_handle(),
            authority: ctx.accounts.mint_authority.cpi_handle(),
        };
        token_cpi::freeze_account(
            CpiContext::new(ctx.accounts.token_program.address(), cpi_accounts)
                .with_signer(&[&signer_seeds]),
        )?;
    } else {
        let cpi_accounts = token_cpi::accounts::ThawAccount {
            account: ctx.accounts.user_franksol_ata.cpi_handle_mut(),
            mint: ctx.accounts.franksol_mint.cpi_handle(),
            authority: ctx.accounts.mint_authority.cpi_handle(),
        };
        token_cpi::thaw_account(
            CpiContext::new(ctx.accounts.token_program.address(), cpi_accounts)
                .with_signer(&[&signer_seeds]),
        )?;
    }

    Ok(())
}
