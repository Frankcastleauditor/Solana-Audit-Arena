use anchor_lang::prelude::*;

pub const SETTLEMENT_AUTHORITY_SEED: &[u8] = b"x402-settlement-authority";

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct Witness {
    pub to: Pubkey,
    pub valid_after: u64,
}
