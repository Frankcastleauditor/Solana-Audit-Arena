use anchor_lang_v2::prelude::*;

#[account]
#[derive(Default)]
#[repr(C)]
pub struct YieldState {
    pub authority: Address,
    pub total_principal: PodU64,
    pub total_yield_paid: PodU64,
    pub last_config_update_ts: PodI64,
    pub apy_bps: PodU16,
    pub state_bump: u8,
    pub vault_bump: u8,
    pub yield_direction_positive: PodBool,
}

#[account]
#[derive(Default)]
#[repr(C)]
pub struct UserPosition {
    pub owner: Address,
    pub principal: PodU64,
    pub accrued_reward: PodU64,
    pub last_update_ts: PodI64,
    pub bump: u8,
}
