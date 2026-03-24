use anchor_lang::prelude::*;
use crate::{constants::CONFIG_PDA_SEED, error::MissionxErrors, state::global_config::Configuration};


#[derive(Accounts)]
pub struct SetConfigAccounts<'info> {
    #[account(
        mut,
        address = config.owner
    )]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_PDA_SEED],
        bump
    )]
    pub config: Box<Account<'info, Configuration>>,

    pub system_program: Program<'info, System>,
}

pub fn set_options(
    ctx: Context<SetConfigAccounts>,
    owner: Option<Pubkey>,
    is_enabled: Option<bool>,
    missionx_payout_min: Option<u64>,
    missionx_payout_max: Option<u64>,
    creation_fee: Option<u64>,
    fee_recipient: Option<Pubkey>,
    token_program: Option<Pubkey>,
    v0: Option<u64>,
    v1: Option<u64>,
    token_player_payout: Option<u64>,
    token_creator_payout: Option<u64>,
    metadata_authority: Option<Pubkey>,
    migration_threshold: Option<u64>,
    migration_fee: Option<u64>,
    executor: Option<Pubkey>,
    ipns_root: Option<[u8; 65]>,
    fail_grace_period: Option<u64>,
    fail_fee: Option<u64>,
    trade_fee_bps: Option<u64>,
) -> Result<()> {
    let cfg = &mut ctx.accounts.config;

    if let Some(owner) = owner { cfg.owner = owner }; 
    if let Some(is_enabled) = is_enabled { cfg.is_enabled = is_enabled }; 
    if let Some(missionx_payout_min) = missionx_payout_min { cfg.missionx_payout_min = missionx_payout_min }; 
    if let Some(missionx_payout_max) = missionx_payout_max { cfg.missionx_payout_max = missionx_payout_max }; 

    require!(cfg.missionx_payout_min <= cfg.missionx_payout_max, MissionxErrors::MissionxPayoutMinLessMaxContraint);

    if let Some(creation_fee) = creation_fee { cfg.creation_fee = creation_fee }; 
    if let Some(fee_recipient) = fee_recipient { cfg.fee_recipient = fee_recipient }; 
    if let Some(token_program) = token_program { cfg.token_program = token_program }; 
    if let Some(v0) = v0 { cfg.v0 = v0 }; 
    if let Some(v1) = v1 { cfg.v1 = v1 }; 
    if let Some(token_player_payout) = token_player_payout { cfg.token_player_payout = token_player_payout }; 
    if let Some(token_creator_payout) = token_creator_payout { cfg.token_creator_payout = token_creator_payout }; 
    cfg.metadata_authority = metadata_authority;
    if let Some(migration_threshold) = migration_threshold { cfg.migration_threshold = migration_threshold }; 
    if let Some(migration_fee) = migration_fee { cfg.migration_fee = migration_fee }; 
    if let Some(executor) = executor { cfg.executor = executor }; 
    if let Some(ipns_root) = ipns_root { cfg.ipns_root = ipns_root }; 
    if let Some(fail_grace_period) = fail_grace_period { cfg.fail_grace_period = fail_grace_period }; 
    if let Some(fail_fee) = fail_fee { cfg.fail_fee = fail_fee }; 
    if let Some(trade_fee_bps) = trade_fee_bps { cfg.trade_fee_bps = trade_fee_bps }; 

    Ok(())
}