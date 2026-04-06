use anchor_lang::prelude::*;

use crate::errors::TokenError;
use crate::events::{BondingCurveCompletedEvent, TradeEvent};
use crate::state::bonding_curve::BondingCurve;
use crate::state::market::Market;
use spl_token_swap::curve::{
    calculator::SwapWithoutFeesResult, constant_product::swap, fees::calculate_fee,
};

use anchor_lang::system_program::{transfer, Transfer};

use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
    token_interface::{transfer_checked, TransferChecked},
};

pub fn buy_tokens(
    ctx: Context<BuyTokens>,
    sol_amount: u64,
    min_token_amount: u64,
    _market_version: u16,
) -> Result<()> {
    let bonding_curve = &mut ctx.accounts.bonding_curve;

    if bonding_curve.completed {
        return Err(TokenError::BondingCurveCompleted.into());
    }

    let token_reserves: u128 = bonding_curve.virtual_token_reserves.into();
    let sol_reserves: u128 = bonding_curve.virtual_sol_reserves.into();
    let sol_amount: u128 = sol_amount.into();

    let SwapWithoutFeesResult {
        source_amount_swapped,
        destination_amount_swapped,
    } = swap(sol_amount, sol_reserves, token_reserves).unwrap();

    let new_sol_amount: u64 = source_amount_swapped.try_into().unwrap();
    let token_amount: u64 = destination_amount_swapped.try_into().unwrap();

    if sol_amount == 0 {
        return Err(TokenError::TokenAmountZero.into());
    }

    if token_amount < min_token_amount {
        return Err(TokenError::MinTokenAmountNotMet.into());
    }
    bonding_curve.real_token_reserves = bonding_curve
        .real_token_reserves
        .checked_sub(token_amount)
        .unwrap();
    bonding_curve.real_sol_reserves = bonding_curve
        .real_sol_reserves
        .checked_add(new_sol_amount)
        .unwrap();
    bonding_curve.virtual_token_reserves = bonding_curve
        .virtual_token_reserves
        .checked_sub(token_amount)
        .unwrap();
    bonding_curve.virtual_sol_reserves = bonding_curve
        .virtual_sol_reserves
        .checked_add(new_sol_amount)
        .unwrap();

    let token_supply = bonding_curve.token_supply;
    let sold_amount = token_supply - bonding_curve.real_token_reserves;
    let market = &ctx.accounts.market;
    if sold_amount >= market.escape_amount {
        bonding_curve.completed = true;
        emit!(BondingCurveCompletedEvent {
            mint: *ctx.accounts.mint.to_account_info().key,
            real_sol_reserve: bonding_curve.real_sol_reserves,
            real_token_reserve: bonding_curve.real_token_reserves,
            virtual_sol_reserve: bonding_curve.virtual_sol_reserves,
            virtual_token_reserve: bonding_curve.virtual_token_reserves,
            timestamp: Clock::get().unwrap().unix_timestamp,
        });
    }

    let transfer_sol_cpi_context = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        Transfer {
            from: ctx.accounts.buyer.to_account_info(),
            to: bonding_curve.to_account_info(),
        },
    );

    transfer(transfer_sol_cpi_context, new_sol_amount)?;

    let accounts = TransferChecked {
        from: ctx.accounts.bonding_curve_ata.to_account_info(),
        to: ctx.accounts.buyer_ata.to_account_info(),
        authority: bonding_curve.to_account_info(),
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

    let trading_fee_bps: u128 = market.trading_fee_bps.into();
    let fee_base: u128 = sol_amount.into();
    let trading_fee = calculate_fee(fee_base, trading_fee_bps, 10000).unwrap() as u64;

    let transfer_fee_cpi_context = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        Transfer {
            from: ctx.accounts.buyer.to_account_info(),
            to: ctx.accounts.trading_fee_treasury.to_account_info(),
        },
    );
    transfer(transfer_fee_cpi_context, trading_fee)?;

    emit!(TradeEvent {
        sol_amount: new_sol_amount,
        token_amount,
        real_sol_reserve: bonding_curve.real_sol_reserves,
        real_token_reserve: bonding_curve.real_token_reserves,
        virtual_sol_reserve: bonding_curve.virtual_sol_reserves,
        virtual_token_reserve: bonding_curve.virtual_token_reserves,
        by: *ctx.accounts.buyer.to_account_info().key,
        mint: *ctx.accounts.mint.to_account_info().key,
        is_buy: true,
        timestamp: Clock::get().unwrap().unix_timestamp,
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(
    sol_amount: u64,
    min_token_amount: u64,
    market_version: u16
)]
pub struct BuyTokens<'info> {
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
        associated_token::authority = bonding_curve,
    )]
    pub bonding_curve_ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub buyer: Signer<'info>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = buyer,
    )]
    pub buyer_ata: Account<'info, TokenAccount>,
    #[account(
        seeds = [b"market".as_ref(), market_version.to_le_bytes().as_ref()],
        bump
    )]
    pub market: Account<'info, Market>,
    /// CHECK: Treasury account for trading fees
    #[account(mut, constraint = market.trading_fee_treasury == trading_fee_treasury.key())]
    pub trading_fee_treasury: AccountInfo<'info>,
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}
