use anchor_lang::prelude::*;

#[event]
pub struct Settled {
    pub eth_address: [u8; 20],
    pub to: Pubkey,
    pub amount: u64,
    pub nonce: u64,
}
