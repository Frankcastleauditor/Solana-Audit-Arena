use anchor_lang::prelude::*;

use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

pub fn init_ata(_ctx: Context<InitAta>) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
pub struct InitAta<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: somebody else can create the associated token account
    pub authority: AccountInfo<'info>,
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = authority,
    )]
    pub payer_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}
