use anchor_lang::prelude::*;

#[event]
pub struct Deposited {
    pub eth_address: [u8; 20],
    pub mint: Pubkey,
    pub depositor: Pubkey,
    pub amount: u64,
}

#[event]
pub struct PermitTransferExecuted {
    pub eth_address: [u8; 20],
    pub mint: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
    pub nonce: u64,
}

#[event]
pub struct UnorderedNonceInvalidation {
    pub eth_address: [u8; 20],
    pub word_pos: u64,
    pub mask_lo: u128,
    pub mask_hi: u128,
}

#[event]
pub struct NonceBitmapClosed {
    pub eth_address: [u8; 20],
    pub word_pos: u64,
    pub rent_payer: Pubkey,
}
