use anchor_lang::prelude::*;

#[account]
pub struct BondingCurve {
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub token_supply: u64,
    pub completed: bool,
    pub tokens_fee_cooldown_timestamp: i64,
    pub market: Pubkey,
}
