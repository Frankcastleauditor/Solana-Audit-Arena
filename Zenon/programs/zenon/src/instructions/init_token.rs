use anchor_lang::prelude::*;

use crate::events::InitTokenEvent;
use crate::state::bonding_curve::BondingCurve;
use crate::state::market::Market;
use anchor_spl::{
    associated_token::AssociatedToken,
    metadata::{
        create_metadata_accounts_v3, mpl_token_metadata::types::DataV2, CreateMetadataAccountsV3,
        Metadata as Metaplex,
    },
    token::{self, Mint, Token, TokenAccount},
};

use std::mem::size_of;

pub fn init_token(ctx: Context<InitializeAndMint>, params: InitTokenParams) -> Result<()> {
    let market = &ctx.accounts.market;
    let initial_mint = market.initial_mint;
    let mint_to_cpi_context = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        token::MintTo {
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.bonding_curve_ata.to_account_info(),
            authority: ctx.accounts.payer.to_account_info(),
        },
    );
    token::mint_to(mint_to_cpi_context, initial_mint)?;

    let virtual_token_reserves = params.token_offset.checked_add(initial_mint).unwrap();
    ctx.accounts.bonding_curve.real_token_reserves = initial_mint;
    ctx.accounts.bonding_curve.real_sol_reserves = 0;
    ctx.accounts.bonding_curve.virtual_token_reserves = virtual_token_reserves;
    ctx.accounts.bonding_curve.virtual_sol_reserves = params.sol_offset;
    ctx.accounts.bonding_curve.token_supply = initial_mint;
    ctx.accounts.bonding_curve.completed = false;
    ctx.accounts.bonding_curve.market = market.key();
    ctx.accounts.bonding_curve.tokens_fee_cooldown_timestamp = params.tokens_fee_cooldown_timestamp;

    let token_data: DataV2 = DataV2 {
        name: params.name,
        symbol: params.symbol,
        uri: params.uri,
        seller_fee_basis_points: 0,
        creators: None,
        collection: None,
        uses: None,
    };

    let metadata_ctx = CpiContext::new(
        ctx.accounts.token_metadata_program.to_account_info(),
        CreateMetadataAccountsV3 {
            metadata: ctx.accounts.metadata.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            payer: ctx.accounts.payer.to_account_info(),
            update_authority: ctx.accounts.mint.to_account_info(),
            mint_authority: ctx.accounts.payer.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            rent: ctx.accounts.rent.to_account_info(),
        },
    );

    create_metadata_accounts_v3(metadata_ctx, token_data, false, true, None)?;

    let set_authority_cpi_context = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        token::SetAuthority {
            account_or_mint: ctx.accounts.mint.to_account_info(),
            current_authority: ctx.accounts.payer.to_account_info(),
        },
    );
    token::set_authority(
        set_authority_cpi_context,
        spl_token::instruction::AuthorityType::MintTokens,
        None,
    )?;

    emit!(InitTokenEvent {
        mint: *ctx.accounts.mint.to_account_info().key,
        virtual_token_reserve: virtual_token_reserves,
        virtual_sol_reserve: params.sol_offset,
        real_token_reserve: initial_mint,
        real_sol_reserve: 0,
        timestamp: Clock::get().unwrap().unix_timestamp,
    });

    Ok(())
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone)]
pub struct InitTokenParams {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub decimals: u8,
    pub sol_offset: u64,
    pub token_offset: u64,
    pub tokens_fee_cooldown_timestamp: i64,
}

#[derive(Accounts)]
#[instruction(
    params: InitTokenParams
)]
pub struct InitializeAndMint<'info> {
    #[account(
        init,
        payer = payer,
        mint::decimals = params.decimals,
        mint::authority = payer,
    )]
    pub mint: Account<'info, Mint>,
    #[account(
        init,
        seeds = [b"bonding_curve".as_ref(), mint.key().as_ref()],
        bump,
        payer = payer,
        space = size_of::<BondingCurve>() + 8,
    )]
    pub bonding_curve: Account<'info, BondingCurve>,
    pub market: Account<'info, Market>,
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = bonding_curve,
     )]
    pub bonding_curve_ata: Account<'info, TokenAccount>,
    /// CHECK: New Metaplex Account being created
    #[account(mut)]
    pub metadata: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
    pub token_metadata_program: Program<'info, Metaplex>,
}
