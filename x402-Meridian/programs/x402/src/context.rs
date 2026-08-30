use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use permit2::program::Permit2;
use permit2::{PermitTransferFromArgs, Vault};

use crate::state::SETTLEMENT_AUTHORITY_SEED;

#[derive(Accounts)]
#[instruction(permit: PermitTransferFromArgs, eth_address: [u8; 20])]
pub struct Settle<'info> {
    #[account(mut)]
    pub facilitator: Signer<'info>,

    /// CHECK: not read, only used as a CPI signer via `invoke_signed`.
    #[account(seeds = [SETTLEMENT_AUTHORITY_SEED], bump)]
    pub settlement_authority: UncheckedAccount<'info>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub vault: Account<'info, Vault>,

    #[account(mut)]
    pub vault_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub recipient_token_account: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: forwarded to permit2 via CPI, which applies `init_if_needed` plus its
    /// own seeds/bump constraints. Left untyped here because on a nonce word's
    /// first-ever use this account does not exist yet — a typed `Account<'info, _>`
    /// extractor would fail to deserialize it before the CPI ever runs.
    #[account(mut)]
    pub nonce_bitmap: UncheckedAccount<'info>,

    /// CHECK: validated by permit2 via CPI; only forwarded here.
    pub instructions_sysvar: UncheckedAccount<'info>,

    pub permit2_program: Program<'info, Permit2>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
