use anchor_lang::prelude::*;

use crate::state::bonding_curve::BondingCurve;
use crate::{errors::TokenError, state::market::Market};
use anchor_spl::{
    token::{Mint, Token, TokenAccount},
    token_interface::{transfer_checked, TransferChecked},
};

pub fn withdraw_tokens_fee(
    ctx: Context<WithdrawTokensFee>,
    tokens_amount: u64,
    _market_version: u16,
) -> Result<()> {
    let bonding_curve = &ctx.accounts.bonding_curve;
    if !bonding_curve.completed {
        return Err(TokenError::BondingCurveNotCompleted.into());
    }
    if Clock::get().unwrap().unix_timestamp
        < ctx.accounts.bonding_curve.tokens_fee_cooldown_timestamp
    {
        return Err(TokenError::TokensFeeCooldown.into());
    }

    let accounts = TransferChecked {
        from: ctx.accounts.bonding_curve_ata.to_account_info(),
        to: ctx.accounts.treasury_ata.to_account_info(),
        authority: ctx.accounts.bonding_curve.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
    };
    let bump = ctx.bumps.bonding_curve;
    let seeds = &[
        b"bonding_curve".as_ref(),
        ctx.accounts.mint.to_account_info().key.as_ref(),
        &[bump],
    ];

    let signer_seeds = &[&seeds[..]];

    let transfer_tokens_ctx_cpi = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        accounts,
        signer_seeds,
    );

    transfer_checked(
        transfer_tokens_ctx_cpi,
        tokens_amount,
        ctx.accounts.mint.decimals,
    )
    .map_err(|_| TokenError::TransferError)?;
    Ok(())
}

#[derive(Accounts)]
#[instruction(
    tokens_amount: u64,
    market_version: u16
)]
pub struct WithdrawTokensFee<'info> {
    #[account(
        mut,
        seeds = [b"bonding_curve".as_ref(), mint.key().as_ref()],
        bump,
        constraint = bonding_curve.market == market.key()
    )]
    pub bonding_curve: Account<'info, BondingCurve>,
    #[account(
        seeds = [b"market".as_ref(), market_version.to_le_bytes().as_ref()],
        bump
    )]
    pub market: Account<'info, Market>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = market.tokens_fee_treasury,
    )]
    pub treasury_ata: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = bonding_curve
    )]
    pub bonding_curve_ata: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = market.authority == admin.key(),
    )]
    pub admin: Signer<'info>,
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}
