use anchor_lang::prelude::*;
use spl_token_swap::curve::fees::calculate_fee;

use crate::state::bonding_curve::BondingCurve;
use crate::{errors::TokenError, state::market::Market};
use anchor_spl::{
    token::{Mint, Token, TokenAccount},
    token_interface::{transfer_checked, TransferChecked},
};

pub fn process_completed_curve(
    ctx: Context<ProcessCompletedCurve>,
    _market_version: u16,
) -> Result<()> {
    let bonding_curve = &ctx.accounts.bonding_curve;
    if !bonding_curve.completed {
        return Err(TokenError::BondingCurveNotCompleted.into());
    }

    let market = &ctx.accounts.market;
    let token_amount = bonding_curve
        .real_token_reserves
        .checked_sub(market.tokens_fee_amount)
        .unwrap();
    let accounts = TransferChecked {
        from: ctx.accounts.bonding_curve_ata.to_account_info(),
        to: ctx.accounts.admin_ata.to_account_info(),
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
        token_amount,
        ctx.accounts.mint.decimals,
    )
    .map_err(|_| TokenError::TransferError)?;

    let escape_fee_sol_amount = calculate_fee(
        bonding_curve.real_sol_reserves.into(),
        market.escape_fee_bps.into(),
        10000,
    )
    .unwrap() as u64;
    let lp_sol_amount = bonding_curve
        .real_sol_reserves
        .checked_sub(escape_fee_sol_amount)
        .unwrap();
    let lp_sol_recipient = &ctx.accounts.admin.to_account_info();
    let escape_fee_recipient = &ctx.accounts.escape_fee_treasury;
    **bonding_curve.to_account_info().try_borrow_mut_lamports()? -= bonding_curve.real_sol_reserves;
    **lp_sol_recipient
        .to_account_info()
        .try_borrow_mut_lamports()? += lp_sol_amount;
    **escape_fee_recipient.try_borrow_mut_lamports()? += escape_fee_sol_amount;
    Ok(())
}

#[derive(Accounts)]
#[instruction(
    market_version: u16
)]
pub struct ProcessCompletedCurve<'info> {
    #[account(
        mut,
        seeds = [b"bonding_curve".as_ref(), mint.key().as_ref()],
        bump,
        constraint = bonding_curve.market == market.key()
    )]
    pub bonding_curve: Account<'info, BondingCurve>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = admin,
    )]
    pub admin_ata: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = bonding_curve,
    )]
    pub bonding_curve_ata: Account<'info, TokenAccount>,
    #[account(
        seeds = [b"market".as_ref(), market_version.to_le_bytes().as_ref()],
        bump
    )]
    pub market: Account<'info, Market>,
    /// CHECK: Treasury account for the escape fee
    #[account(mut, constraint = market.escape_fee_treasury == escape_fee_treasury.key())]
    pub escape_fee_treasury: AccountInfo<'info>,
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
