use anchor_lang::prelude::*;
use crate::{constants::{CONFIG_PDA_SEED, MISSIONX_STATE}, error::MissionxErrors, events::MissionxFailed, state::{missionx::{Missionx, MissionxStatus}, global_config::Configuration}};
use anchor_spl::token_interface;

use super::{ensure_missionx_state, ensure_enabled, refund_payout};

#[derive(Accounts)]
pub struct FailAccounts<'info> {
    #[account(
        mut,
        address = missionx_state.creator
    )]
    pub creator: Signer<'info>,

    #[account(
        seeds = [CONFIG_PDA_SEED],
        bump
    )]
    pub config: Box<Account<'info, Configuration>>,
    
    #[account(
        mut,
        seeds = [MISSIONX_STATE, &token_mint.key().to_bytes()],
        bump
    )]
    pub missionx_state: Box<Account<'info, Missionx>>,

    #[account(
        address = missionx_state.token_mint
    )]
    pub token_mint: InterfaceAccount<'info, token_interface::Mint>,

    /// CHECK: not required
    #[account(
        mut,
        address = config.fee_recipient
    )]
    pub fee_recipient: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FailAccountsByTime<'info> {
    #[account()]
    pub user: Signer<'info>,

    #[account(
        seeds = [CONFIG_PDA_SEED],
        bump
    )]
    pub config: Box<Account<'info, Configuration>>,
    
    #[account(
        mut,
        seeds = [MISSIONX_STATE, &token_mint.key().to_bytes()],
        bump
    )]
    pub missionx_state: Box<Account<'info, Missionx>>,

    #[account(
        address = missionx_state.token_mint
    )]
    pub token_mint: InterfaceAccount<'info, token_interface::Mint>,

    /// CHECK: not required
    #[account(
        mut,
        address = config.fee_recipient
    )]
    pub fee_recipient: AccountInfo<'info>,

    /// CHECK: not required
    #[account(
        mut,
        address = missionx_state.creator
    )]
    pub creator: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}


pub fn fail_missionx(
    ctx: Context<FailAccounts>,
) -> Result<()> {
    ensure_enabled(&ctx.accounts.config)?;
    ensure_missionx_state(&ctx.accounts.missionx_state, MissionxStatus::Open)?;
    
    let clock = Clock::get()?;
    let missionx_state = &mut ctx.accounts.missionx_state;
    missionx_state.missionx_status = MissionxStatus::Failed;
    missionx_state.fail_ts = Some(clock.unix_timestamp as u64);
    
    refund_payout(
        missionx_state,
        &ctx.accounts.config,
        &ctx.accounts.fee_recipient,
        &ctx.accounts.creator
    )?;

    msg!("MissionxFailed; ID: {}, fail_ts: {}, by_creator: {}",
        ctx.accounts.token_mint.key(),
        clock.unix_timestamp as u64,
        true
    );
    emit!(MissionxFailed {
        id: ctx.accounts.token_mint.key(),
        fail_ts: clock.unix_timestamp as u64,
        by_creator: true,
    });


    Ok(())
}

pub fn fail_missionx_by_time(
    ctx: Context<FailAccountsByTime>,
) -> Result<()> {
    ensure_enabled(&ctx.accounts.config)?;
    ensure_missionx_state(&ctx.accounts.missionx_state, MissionxStatus::Open)?;
    
    let clock = Clock::get()?;
    require!(ctx.accounts.missionx_state.open_timestamp + ctx.accounts.missionx_state.open_duration < (clock.unix_timestamp as u64), MissionxErrors::MissionxNotFailedByExpiration);

    let missionx_state = &mut ctx.accounts.missionx_state;
    missionx_state.missionx_status = MissionxStatus::Failed;
    missionx_state.fail_ts = Some(missionx_state.open_timestamp + missionx_state.open_duration);

    refund_payout(
        missionx_state,
        &ctx.accounts.config,
        &ctx.accounts.fee_recipient,
        &ctx.accounts.creator
    )?;

    msg!("MissionxFailed; ID: {}, fail_ts: {}, by_creator: {}",
        ctx.accounts.token_mint.key(),
        clock.unix_timestamp as u64,
        false
    );
    emit!(MissionxFailed {
        id: ctx.accounts.token_mint.key(),
        fail_ts: missionx_state.fail_ts.unwrap(),
        by_creator: false,
    });

    Ok(())
}