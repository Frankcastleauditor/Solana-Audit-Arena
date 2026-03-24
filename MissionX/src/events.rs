use anchor_lang::prelude::*;

use crate::state::missionx::{MissionxStatus, MissionxTradeStatus};


#[event]
pub struct ModeratorUpdate {
    pub moderator: Pubkey,
    pub enabled: bool,
}

#[event]
pub struct MissionxCreated {
    pub id: Pubkey,
    pub creator: Pubkey,
    pub payout: u64,
    pub timestamp: u64,
}

#[event]
pub struct MissionxModerated {
    pub id: Pubkey,
    pub moderator: Pubkey,
    pub new_status: MissionxStatus,
    pub is_block: bool,
}

#[event]
pub struct MissionxBan {
    pub id: Pubkey,
    pub moderator: Pubkey,
    pub new_status: MissionxStatus,
    pub new_trade_status: MissionxTradeStatus,
    pub is_ban: bool,
    pub ban_sell: bool,
}

#[event]
pub struct MissionxBanFailed {
    pub id: Pubkey,
    pub moderator: Pubkey,
    pub immediate: bool,
}


#[event]
pub struct MissionxAccepted {
    pub id: Pubkey,
    pub player: Pubkey,
}


#[event]
pub struct MissionxCompleted {
    pub id: Pubkey,
    pub player: Pubkey,
}

#[event]
pub struct MissionxPlayerFailed {
    pub id: Pubkey,
    pub player: Pubkey,
    pub missionx_state: MissionxStatus,
}

#[event]
pub struct MissionxTrade {
    pub id: Pubkey,
    pub user: Pubkey,
    pub sol_amount: u64,
    pub tkn_amount: u64,
    pub fee: u64,
    pub is_buy: bool,
}

#[event]
pub struct MissionxMigrationRequired {
    pub id: Pubkey,
}

#[event]
pub struct MissionxMigrated {
    pub id: Pubkey,
}

#[event]
pub struct MissionxFailed {
    pub id: Pubkey,
    pub fail_ts: u64,
    pub by_creator: bool,
}

#[event]
pub struct MissionxWithdraw {
    pub id: Pubkey,
}