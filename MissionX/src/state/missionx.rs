use anchor_lang::{prelude::*, solana_program::pubkey::Pubkey};

#[derive(Debug, Clone, Copy, AnchorSerialize, AnchorDeserialize, PartialEq, Eq)]
pub enum MissionxStatus {
    Unverified,
    Censored,
    Open,
    Accepted,
    Completed,
    Failed,
    Withdrawn
}

#[derive(Debug, Clone, Copy, AnchorSerialize, AnchorDeserialize, PartialEq, Eq)]
pub enum MissionxTradeStatus {
    Closed,
    Open,
    MigrationRequired,
    Migrated,
    Banned,
    Withdrawn
}

#[account]
pub struct Missionx {
    pub creator: Pubkey,
    pub payout_amount: u64,
    pub missionx_status: MissionxStatus,
    pub trade_status: MissionxTradeStatus,
    pub token_program: Pubkey,
    pub created_at: u64,
    pub is_blocked: bool,
    pub missionx_cid: [u8; 65],
    pub token_mint: Pubkey,
    pub v0: u64,
    pub v1: u64,
    pub reserve0: u64,
    pub reserve1: u64,
    pub submitters: [Option<Pubkey>; 3],
    pub token_player_payout: u64,
    pub token_creator_payout: u64,
    pub migration_threshold: u64,
    pub migration_fee: u64,
    pub open_timestamp: u64,
    pub success_time: u64,
    pub fail_ts: Option<u64>,
    pub old_trade_status: Option<MissionxTradeStatus>,
    pub open_duration: u64,
}

impl Missionx {
    pub fn get_full_sol_reserve(&self) -> u64 {
        self.v0 + self.reserve0
    }
    pub fn get_full_token_reserve(&self) -> u64 {
        self.v1 + self.reserve1
    }
}