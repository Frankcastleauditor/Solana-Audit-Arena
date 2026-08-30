use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as ix_sysvar;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::state::{
    NonceBitmap, PermitTransferFromArgs, SignatureTransferDetails, Vault, NONCE_BITMAP_SEED,
    VAULT_AUTHORITY_SEED, VAULT_SEED,
};

#[derive(Accounts)]
#[instruction(eth_address: [u8; 20])]
pub struct InitVault<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = payer,
        space = 8 + Vault::INIT_SPACE,
        seeds = [VAULT_SEED, eth_address.as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, Vault>,

    #[account(
        init,
        payer = payer,
        token::mint = mint,
        token::authority = vault,
        seeds = [VAULT_AUTHORITY_SEED, eth_address.as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub vault_token_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(eth_address: [u8; 20])]
pub struct Deposit<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        seeds = [VAULT_SEED, eth_address.as_ref(), mint.key().as_ref()],
        bump = vault.bump,
        has_one = mint,
    )]
    pub vault: Account<'info, Vault>,

    #[account(mut, address = vault.token_account)]
    pub vault_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub depositor_token_account: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
#[instruction(permit: PermitTransferFromArgs, transfer_details: SignatureTransferDetails, eth_address: [u8; 20])]
pub struct PermitTransfer<'info> {
    pub spender: Signer<'info>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        seeds = [VAULT_SEED, eth_address.as_ref(), mint.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, Vault>,

    #[account(mut, address = vault.token_account)]
    pub vault_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub recipient_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = fee_payer,
        space = 8 + NonceBitmap::INIT_SPACE,
        seeds = [NONCE_BITMAP_SEED, eth_address.as_ref(), &(permit.nonce / 256).to_le_bytes()],
        bump,
    )]
    pub nonce_bitmap: Account<'info, NonceBitmap>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    /// CHECK: read via the instructions sysvar loader, which validates the address itself.
    #[account(address = ix_sysvar::ID)]
    pub instructions_sysvar: AccountInfo<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(eth_address: [u8; 20], word_pos: u64)]
pub struct InvalidateNonces<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + NonceBitmap::INIT_SPACE,
        seeds = [NONCE_BITMAP_SEED, eth_address.as_ref(), &word_pos.to_le_bytes()],
        bump,
    )]
    pub nonce_bitmap: Account<'info, NonceBitmap>,

    /// CHECK: read via the instructions sysvar loader, which validates the address itself.
    #[account(address = ix_sysvar::ID)]
    pub instructions_sysvar: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(eth_address: [u8; 20], word_pos: u64)]
pub struct CloseNonceBitmap<'info> {
    #[account(
        mut,
        seeds = [NONCE_BITMAP_SEED, eth_address.as_ref(), &word_pos.to_le_bytes()],
        bump = nonce_bitmap.bump,
        close = rent_payer,
    )]
    pub nonce_bitmap: Account<'info, NonceBitmap>,

    /// CHECK: rent lamports are simply credited here; identity is pinned by `address`.
    #[account(mut, address = nonce_bitmap.rent_payer)]
    pub rent_payer: UncheckedAccount<'info>,
}
