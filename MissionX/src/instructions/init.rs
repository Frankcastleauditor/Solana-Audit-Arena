use anchor_lang::prelude::*;
use std::mem::size_of;

use crate::{constants::{BPS, CONFIG_PDA_SEED, MINT_AMOUNT}, error::MissionxErrors, state::global_config::Configuration};

#[derive(Accounts)]
pub struct InitAccounts<'info> {
    #[account(
        init,
        payer = owner,
        seeds = [CONFIG_PDA_SEED],
        bump,
        space = 8 + size_of::<Configuration>(),
        rent_exempt = enforce
    )]
    pub config: Box<Account<'info, Configuration>>,

    // TODO: ensure that owner is our wallet
    #[account(mut)]
    pub owner: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn init(
    ctx: Context<InitAccounts>,
    is_enabled: bool,
    missionx_payout_min: u64,
    missionx_payout_max: u64,
    creation_fee: u64,
    fee_recipient: Pubkey,
    token_program: Pubkey,
    v0: u64,
    v1: u64,
    metadata_authority: Option<Pubkey>,
    executor: Pubkey,
    ipns_root: [u8; 65],
) -> Result<()> {
    let config: &mut Box<Account<'_, Configuration>> = &mut ctx.accounts.config;

    require!(config.initialized == false, MissionxErrors::AlreadyInitialized);
    require!(missionx_payout_min <= missionx_payout_max, MissionxErrors::MissionxPayoutMinLessMaxContraint);
    require!(ipns_root.len() == 65, MissionxErrors::IpnsContrain);

    config.missionx_payout_min = missionx_payout_min;
    config.missionx_payout_max = missionx_payout_max;
    config.creation_fee = creation_fee;
    config.fee_recipient = fee_recipient;
    config.token_program = token_program;
    config.v0 = v0;
    config.v1 = v1;
    config.token_player_payout = ((1_00 as u128 * MINT_AMOUNT as u128) / BPS as u128) as u64;
    config.token_creator_payout = ((0_50 as u128 * MINT_AMOUNT as u128) / BPS as u128) as u64;
    config.metadata_authority = metadata_authority;
    config.migration_threshold = 822 * 1_000_000_000 / 10;
    config.migration_fee = 5 * 1_000_000_000;
    config.executor = executor;
    config.ipns_root.copy_from_slice(ipns_root.as_slice());
    config.fail_grace_period = 30 * 24 * 60 * 60;
    config.fail_fee = 5 * 10_000_000;
    config.trade_fee_bps = 1_00;


    config.is_enabled = is_enabled;
    config.owner = *ctx.accounts.owner.key;
    config.initialized = true;
  
    Ok(())
}