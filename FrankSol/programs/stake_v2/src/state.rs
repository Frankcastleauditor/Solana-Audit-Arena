use anchor_lang_v2::borsh::{BorshDeserialize, BorshSerialize};
use anchor_lang_v2::prelude::*;
use bytemuck::{Pod, Zeroable};

#[derive(Pod, Zeroable, Clone, Copy, Default)]
#[repr(C)]
pub struct FeeRecipient {
    pub pubkey: Address,
    pub fee_bps: PodU16,
}

#[derive(
    BorshSerialize,
    BorshDeserialize,
    anchor_lang_v2::wincode::SchemaRead,
    anchor_lang_v2::wincode::SchemaWrite,
    Clone,
    Copy,
)]
pub struct FeeRecipientArg {
    pub pubkey: Address,
    pub fee_bps: u16,
}

#[account]
#[derive(Default)]
#[repr(C)]
pub struct Pool {
    pub admin: Address,
    pub fund_manager: Address,
    pub franksol_mint: Address,
    pub fee_recipients: [FeeRecipient; 3],
    pub total_sol: PodU64,
    pub deployed_sol: PodU64,
    pub franksol_supply: PodU64,
    pub bump: u8,
    pub vault_bump: u8,
    pub mint_auth_bump: u8,
}

#[account]
#[derive(Default)]
#[repr(C)]
pub struct UserPosition {
    pub owner: Address,
    pub franksol_balance: PodU64,
    pub sol_deposited: PodU64,
    pub is_blacklisted: PodBool,
    pub bump: u8,
}
