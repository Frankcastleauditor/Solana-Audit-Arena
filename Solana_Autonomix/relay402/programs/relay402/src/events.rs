use anchor_lang::prelude::*;

#[event]
pub struct ConfigInitialized {
    pub admin: Pubkey,
    pub mint: Pubkey,
    pub treasury: Pubkey,
    pub fee_bps: u16,
    pub min_price: u64,
}

#[event]
pub struct ConfigUpdated {
    pub fee_bps: u16,
    pub min_price: u64,
    pub paused: bool,
    pub treasury: Pubkey,
}

#[event]
pub struct AdminTransferProposed {
    pub admin: Pubkey,
    pub pending_admin: Pubkey,
}

#[event]
pub struct AdminTransferred {
    pub old_admin: Pubkey,
    pub new_admin: Pubkey,
}

#[event]
pub struct AgentRegistered {
    pub agent: Pubkey,
    pub id: u64,
    pub owner: Pubkey,
    pub operator: Pubkey,
    pub payout: Pubkey,
    pub price: u64,
    pub endpoint: String,
    pub metadata_uri: String,
    pub metadata_hash: [u8; 32],
}

#[event]
pub struct AgentUpdated {
    pub agent: Pubkey,
    pub operator: Pubkey,
    pub payout: Pubkey,
    pub price: u64,
    pub endpoint: String,
    pub metadata_uri: String,
    pub metadata_hash: [u8; 32],
    pub active: bool,
}

#[event]
pub struct AgentOwnershipProposed {
    pub agent: Pubkey,
    pub owner: Pubkey,
    pub pending_owner: Pubkey,
}

#[event]
pub struct AgentOwnershipTransferred {
    pub agent: Pubkey,
    pub old_owner: Pubkey,
    pub new_owner: Pubkey,
}

#[event]
pub struct AgentClosed {
    pub agent: Pubkey,
    pub id: u64,
}

#[event]
pub struct PaymentCreated {
    pub receipt: Pubkey,
    pub agent: Pubkey,
    pub client: Pubkey,
    pub nonce: u64,
    pub amount: u64,
    pub fee_bps: u16,
    pub expires_at: i64,
}

#[event]
pub struct PaymentSettled {
    pub receipt: Pubkey,
    pub agent: Pubkey,
    pub client: Pubkey,
    pub amount: u64,
    pub fee: u64,
    pub net: u64,
}

#[event]
pub struct PaymentRefunded {
    pub receipt: Pubkey,
    pub agent: Pubkey,
    pub client: Pubkey,
    pub amount: u64,
}

#[event]
pub struct FeedbackSubmitted {
    pub receipt: Pubkey,
    pub agent: Pubkey,
    pub client: Pubkey,
    pub score: u8,
    pub feedback_count: u64,
    pub score_sum: u64,
}
