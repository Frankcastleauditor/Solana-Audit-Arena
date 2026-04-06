use anchor_lang::prelude::*;

#[event]
pub struct TradeEvent {
    pub sol_amount: u64,
    pub token_amount: u64,
    pub virtual_token_reserve: u64,
    pub virtual_sol_reserve: u64,
    pub real_token_reserve: u64,
    pub real_sol_reserve: u64,
    pub by: Pubkey,
    pub mint: Pubkey,
    pub is_buy: bool,
    pub timestamp: i64,
}

#[event]
pub struct InitTokenEvent {
    pub mint: Pubkey,
    pub timestamp: i64,
    pub virtual_token_reserve: u64,
    pub virtual_sol_reserve: u64,
    pub real_token_reserve: u64,
    pub real_sol_reserve: u64,
}

#[event]
pub struct BondingCurveCompletedEvent {
    pub mint: Pubkey,
    pub virtual_token_reserve: u64,
    pub virtual_sol_reserve: u64,
    pub real_token_reserve: u64,
    pub real_sol_reserve: u64,
    pub timestamp: i64,
}
