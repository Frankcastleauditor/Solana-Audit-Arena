use anchor_lang::prelude::*;

use crate::constants::*;

/// Global protocol configuration. PDA: ["config"].
#[account]
#[derive(InitSpace)]
pub struct Config {
    pub admin: Pubkey,
    /// Two-step admin rotation. Set by `propose_admin`, consumed by `accept_admin`.
    pub pending_admin: Option<Pubkey>,
    /// Payment token (USDC). Classic SPL Token only.
    pub mint: Pubkey,
    /// Token account (mint = `mint`) that receives protocol fees.
    pub treasury: Pubkey,
    /// Shared escrow token account. PDA: ["vault"], authority = config PDA.
    pub vault: Pubkey,
    pub fee_bps: u16,
    /// Lowest price an agent may charge, in base units of `mint`.
    pub min_price: u64,
    /// Monotonic counter used as the agent id. Ids are never reused.
    pub next_agent_id: u64,
    /// When paused, no new agents and no new payments. Settle, refund and
    /// feedback stay open so escrowed funds can always leave the vault.
    pub paused: bool,
    pub bump: u8,
    pub vault_bump: u8,
}

/// Registered agent (the ERC-8004 identity equivalent). PDA: ["agent", id_le].
#[account]
#[derive(InitSpace)]
pub struct Agent {
    pub id: u64,
    pub owner: Pubkey,
    /// Two-step ownership transfer.
    pub pending_owner: Option<Pubkey>,
    /// Hot key used by the agent's HTTP server to sign settlements.
    pub operator: Pubkey,
    /// Token account (mint = config.mint) that receives payments.
    pub payout: Pubkey,
    /// Price per request in base units of config.mint.
    pub price: u64,
    #[max_len(MAX_ENDPOINT_LEN)]
    pub endpoint: String,
    #[max_len(MAX_METADATA_URI_LEN)]
    pub metadata_uri: String,
    /// sha256 of the metadata document. Lets clients detect a swapped document.
    pub metadata_hash: [u8; 32],
    pub active: bool,
    /// Receipts in Pending state. The agent cannot be closed while this is > 0.
    pub pending_receipts: u64,
    pub settled_count: u64,
    pub feedback_count: u64,
    /// Sum of all scores. Average = score_sum / feedback_count (computed off-chain).
    pub score_sum: u64,
    pub created_at: i64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum ReceiptStatus {
    Pending,
    Settled,
}

/// One paid request. PDA: ["receipt", agent, client, nonce_le].
/// Refunded receipts are closed. Settled receipts are closed on feedback or
/// by `close_receipt`.
#[account]
#[derive(InitSpace)]
pub struct Receipt {
    pub agent: Pubkey,
    pub client: Pubkey,
    pub nonce: u64,
    pub amount: u64,
    /// Fee rate snapshot taken at payment time, so later fee changes never
    /// apply to money already in escrow.
    pub fee_bps: u16,
    pub created_at: i64,
    pub expires_at: i64,
    pub settled_at: i64,
    pub status: ReceiptStatus,
    pub bump: u8,
}
