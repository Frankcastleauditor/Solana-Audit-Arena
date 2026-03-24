use anchor_lang::{prelude::*, solana_program::pubkey::Pubkey};

#[account]
pub struct ModeratorState {
    pub is_enabled: bool,
}