use anchor_lang::prelude::*;

#[account]
pub struct Market {
    pub version: u16,
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
