use anchor_lang::{prelude::*, solana_program::pubkey::Pubkey};

#[account]
pub struct Configuration {
    pub initialized: bool,
    pub owner: Pubkey,
    pub is_enabled: bool,
    pub missionx_payout_min: u64,
    pub missionx_payout_max: u64,
    pub creation_fee: u64,
    pub fee_recipient: Pubkey,
    pub token_program: Pubkey,
    pub v0: u64,
    pub v1: u64,
    pub token_player_payout: u64,
    pub token_creator_payout: u64,
    pub metadata_authority: Option<Pubkey>,
    pub migration_threshold: u64,
    pub migration_fee: u64,
    pub executor: Pubkey,
    pub ipns_root: [u8; 65],
    pub fail_grace_period: u64,
    pub fail_fee: u64,
    pub trade_fee_bps: u64,
}

impl Configuration {
    pub fn get_token_reserved(&self) -> u64 {
        self.token_creator_payout + self.token_player_payout
    }
}