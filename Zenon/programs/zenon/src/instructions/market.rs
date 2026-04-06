use anchor_lang::prelude::*;

use crate::{errors::TokenError, state::market::Market};
use std::mem::size_of;

fn save_market<'info>(market: &mut Account<Market>, market_data: MarketParams) -> Result<()> {
    market.authority = market_data.authority;
    market.trading_fee_treasury = market_data.trading_fee_treasury;
    if market_data.trading_fee_bps > 10000 {
        return Err(TokenError::FeeBpsTooHigh.into());
    }
    market.trading_fee_bps = market_data.trading_fee_bps;
    if market_data.tokens_fee_amount >= market_data.escape_amount {
        return Err(TokenError::TokensBpsTooHigh.into());
    }
    market.tokens_fee_amount = market_data.tokens_fee_amount;
    market.tokens_fee_treasury = market_data.tokens_fee_treasury;
    market.escape_fee_treasury = market_data.escape_fee_treasury;
    if market_data.escape_fee_bps > 10000 {
        return Err(TokenError::FeeBpsTooHigh.into());
    }
    market.escape_fee_bps = market_data.escape_fee_bps;
    market.initial_mint = market_data.initial_mint;
    if market.escape_amount > market.initial_mint {
        return Err(TokenError::EscapeAmountTooHigh.into());
    }
    if market.initial_mint == 0 {
        return Err(TokenError::EscapeAmountZero.into());
    }
    market.escape_amount = market_data.escape_amount;
    Ok(())
}

pub fn init_market(
    ctx: Context<InitializeMarket>,
    version: u16,
    market_data: MarketParams,
) -> Result<()> {
    let market = &mut ctx.accounts.market;
    market.version = version;
    save_market(market, market_data)?;
    Ok(())
}

pub fn update_market(
    ctx: Context<UpdateMarket>,
    _version: u16,
    market_data: MarketParams,
) -> Result<()> {
    let market = &mut ctx.accounts.market;
    save_market(market, market_data)?;
    Ok(())
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone)]
pub struct MarketParams {
    pub authority: Pubkey,
    pub initial_mint: u64,
    pub escape_amount: u64,
    pub escape_fee_bps: u16,
    pub escape_fee_treasury: Pubkey,
    pub trading_fee_bps: u16,
    pub trading_fee_treasury: Pubkey,
    pub tokens_fee_amount: u64,
    pub tokens_fee_treasury: Pubkey,
}

#[derive(Accounts)]
#[instruction(
    version: u16,
    market_data: MarketParams
)]
pub struct InitializeMarket<'info> {
    #[account(
        init,
        payer = payer,
        space = size_of::<Market>() + 8,
        seeds = [b"market".as_ref(), version.to_le_bytes().as_ref()],
        bump
    )]
    pub market: Account<'info, Market>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(
    version: u16,
    market_data: MarketParams
)]
pub struct UpdateMarket<'info> {
    #[account(
        mut,
        seeds = [b"market".as_ref(), version.to_le_bytes().as_ref()],
        bump
    )]
    pub market: Account<'info, Market>,
    #[account(
        mut,
        constraint = market.authority == authority.key()
    )]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}
