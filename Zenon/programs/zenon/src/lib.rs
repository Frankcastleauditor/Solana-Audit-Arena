pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;
use instructions::*;

declare_id!("SLAPUqHm76SGThDr4JUyRDVsGBmxwGkxicWwknkgH5c");

#[program]
pub mod zenon {

    use super::*;

    pub fn initialize_market(
        ctx: Context<InitializeMarket>,
        version: u16,
        market_data: MarketParams,
    ) -> Result<()> {
        return instructions::init_market(ctx, version, market_data);
    }

    pub fn update_market(
        ctx: Context<UpdateMarket>,
        version: u16,
        market_data: MarketParams,
    ) -> Result<()> {
        return instructions::update_market(ctx, version, market_data);
    }

    pub fn process_completed_curve(
        ctx: Context<ProcessCompletedCurve>,
        market_version: u16,
    ) -> Result<()> {
        return instructions::process_completed_curve(ctx, market_version);
    }

    pub fn withdraw_tokens_fee(
        ctx: Context<WithdrawTokensFee>,
        tokens_amount: u64,
        market_version: u16,
    ) -> Result<()> {
        return instructions::withdraw_tokens_fee(ctx, tokens_amount, market_version);
    }

    pub fn init_token(ctx: Context<InitializeAndMint>, metadata: InitTokenParams) -> Result<()> {
        return instructions::init_token(ctx, metadata);
    }

    pub fn init_ata(ctx: Context<InitAta>) -> Result<()> {
        return instructions::init_ata(ctx);
    }

    pub fn buy_tokens(
        ctx: Context<BuyTokens>,
        sol_amount: u64,
        min_token_amount: u64,
        market_version: u16,
    ) -> Result<()> {
        return instructions::buy_tokens(ctx, sol_amount, min_token_amount, market_version);
    }

    pub fn sell_tokens(
        ctx: Context<Sell>,
        token_amount: u64,
        min_sol_amount: u64,
        market_version: u16,
    ) -> Result<()> {
        return instructions::sell(ctx, token_amount, min_sol_amount, market_version);
    }
}
