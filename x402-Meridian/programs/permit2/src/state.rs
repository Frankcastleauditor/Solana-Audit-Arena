use anchor_lang::prelude::*;

pub const VAULT_SEED: &[u8] = b"vault";
pub const VAULT_AUTHORITY_SEED: &[u8] = b"vault-authority";
pub const NONCE_BITMAP_SEED: &[u8] = b"nonce-bitmap";

#[account]
#[derive(InitSpace)]
pub struct Vault {
    pub eth_address: [u8; 20],
    pub mint: Pubkey,
    pub token_account: Pubkey,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct NonceBitmap {
    pub eth_address: [u8; 20],
    pub word_pos: u64,
    pub word_lo: u128,
    pub word_hi: u128,
    pub bump: u8,
    pub rent_payer: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct PermitTransferFromArgs {
    pub permitted_token: Pubkey,
    pub permitted_amount: u64,
    pub nonce: u64,
    pub deadline: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct SignatureTransferDetails {
    pub to: Pubkey,
    pub requested_amount: u64,
}
