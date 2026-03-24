use anchor_lang::prelude::*;
use anchor_spl::token_interface;

use crate::{constants::{CONFIG_PDA_SEED, MISSIONX_MODERATOR, MISSIONX_STATE}, error::MissionxErrors, events::{MissionxBan, MissionxBanFailed, MissionxModerated}, instructions::{ensure_missionx_state, ensure_enabled}, state::{missionx::{Missionx, MissionxStatus, MissionxTradeStatus}, global_config::Configuration, moderator::ModeratorState}};

use super::{ensure_moderator_enabled, refund_payout};


#[derive(Accounts)]
pub struct ModearateAccounts<'info> {
    #[account(mut)]
    pub moderator: Signer<'info>,

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
    pub missionx_creator: AccountInfo<'info>,
    
    #[account(
        seeds = [MISSIONX_MODERATOR, &moderator.key.to_bytes()],
        bump
    )]
    pub moderator_config: Box<Account<'info, ModeratorState>>,

    #[account(
        address = missionx_state.token_mint
    )]
    pub token_mint: InterfaceAccount<'info, token_interface::Mint>,


    pub system_program: Program<'info, System>,
}

pub fn moderate_initial(
    ctx: Context<ModearateAccounts>,
    block: bool,
) -> Result<()> {
    ensure_enabled(&ctx.accounts.config)?;
    ensure_missionx_state(&ctx.accounts.missionx_state, MissionxStatus::Unverified)?;
    ensure_moderator_enabled(&ctx.accounts.moderator_config)?;
    
    let clock = Clock::get()?;
    let missionx_state = &mut ctx.accounts.missionx_state;

    if block {
        missionx_state.missionx_status = MissionxStatus::Censored;
        
        refund_payout(
            missionx_state,
            &ctx.accounts.config,
            &ctx.accounts.fee_recipient,
            &ctx.accounts.missionx_creator
        )?;
    } else {
        missionx_state.missionx_status = MissionxStatus::Open;
        missionx_state.open_timestamp = clock.unix_timestamp as u64;
    }

    msg!(
        "MissionxModerated; ID: {}, moderator: {}, new_status:{:?}, is_block: {}",
        ctx.accounts.token_mint.key(),
        ctx.accounts.moderator.key(),
        missionx_state.missionx_status,
        block
    );
    emit!(MissionxModerated{
        id: ctx.accounts.token_mint.key(),
        moderator: ctx.accounts.moderator.key(),
        new_status: missionx_state.missionx_status,
        is_block: block
    });

    Ok(())
}

pub fn ban_active(
    ctx: Context<ModearateAccounts>,
    ban_sell: bool,
) -> Result<()> {
    ensure_enabled(&ctx.accounts.config)?;
    ensure_moderator_enabled(&ctx.accounts.moderator_config)?;
    require!(ctx.accounts.missionx_state.missionx_status != MissionxStatus::Unverified, MissionxErrors::WrongMissionxSatus);

    let missionx_state = &mut ctx.accounts.missionx_state;

    missionx_state.is_blocked = true;
    if ban_sell {
        match missionx_state.trade_status {
            MissionxTradeStatus::Closed |
                MissionxTradeStatus::Open |
                MissionxTradeStatus::MigrationRequired => {
                    if missionx_state.old_trade_status.is_none() {
                        missionx_state.old_trade_status = Some(missionx_state.trade_status);
                    }
                    missionx_state.trade_status = MissionxTradeStatus::Banned
                },
            MissionxTradeStatus::Banned |
                MissionxTradeStatus::Migrated |
                MissionxTradeStatus::Withdrawn => (),
        }
    }

    msg!(
        "MissionxBan; ID: {}, moderator: {}, new_status:{:?}, new_trade_status: {:?}, is_ban: true, ban_sell: {}",
        ctx.accounts.token_mint.key(),
        ctx.accounts.moderator.key(),
        missionx_state.missionx_status,
        missionx_state.trade_status,
        ban_sell
    );
    emit!(MissionxBan {
        id: ctx.accounts.token_mint.key(),
        moderator: ctx.accounts.moderator.key(),
        new_status: missionx_state.missionx_status,
        new_trade_status: missionx_state.trade_status,
        is_ban: true,
        ban_sell
    });

    Ok(())
}

pub fn unban_active(
    ctx: Context<ModearateAccounts>,
) -> Result<()> {
    ensure_enabled(&ctx.accounts.config)?;
    ensure_moderator_enabled(&ctx.accounts.moderator_config)?;
    require!(ctx.accounts.missionx_state.missionx_status != MissionxStatus::Unverified, MissionxErrors::WrongMissionxSatus);
    require!(ctx.accounts.missionx_state.is_blocked, MissionxErrors::MissionxUnblocked);
    if ctx.accounts.missionx_state.old_trade_status.is_none() {
        require!(ctx.accounts.missionx_state.trade_status != MissionxTradeStatus::Banned, MissionxErrors::MissionxTradeBannedNoRecovery);
    }

    let missionx_state = &mut ctx.accounts.missionx_state;

    missionx_state.is_blocked = false;
    if missionx_state.old_trade_status.is_some() {
        missionx_state.trade_status = missionx_state.old_trade_status.unwrap();
    }
    missionx_state.old_trade_status = None;

    msg!(
        "MissionxBan; ID: {}, moderator: {}, new_status:{:?}, new_trade_status: {:?}, is_ban: false, ban_sell: false",
        ctx.accounts.token_mint.key(),
        ctx.accounts.moderator.key(),
        missionx_state.missionx_status,
        missionx_state.trade_status
    );
    emit!(MissionxBan {
        id: ctx.accounts.token_mint.key(),
        moderator: ctx.accounts.moderator.key(),
        new_status: missionx_state.missionx_status,
        new_trade_status: missionx_state.trade_status,
        is_ban: false,
        ban_sell: false
    });

    Ok(())
}

pub fn switch_ban_to_failed(
    ctx: Context<ModearateAccounts>,
    immediate: bool
) -> Result<()> {
    ensure_enabled(&ctx.accounts.config)?;
    ensure_moderator_enabled(&ctx.accounts.moderator_config)?;
    require!(ctx.accounts.missionx_state.is_blocked, MissionxErrors::MissionxUnblocked);
    require!(ctx.accounts.missionx_state.old_trade_status.map_or(true, |s| s == MissionxTradeStatus::Open), MissionxErrors::MissionxNotTradable);

    let clock = Clock::get()?;
    let missionx_state = &mut ctx.accounts.missionx_state;
    missionx_state.missionx_status = MissionxStatus::Failed;
    missionx_state.fail_ts = if immediate { Some(0) } else { Some(clock.unix_timestamp as u64) };
    missionx_state.trade_status = MissionxTradeStatus::Open;
    missionx_state.old_trade_status = None;
    ctx.accounts.missionx_state.is_blocked = false;

    msg!(
        "MissionxBanFailed; ID: {}, moderator: {}, immediate: {}",
        ctx.accounts.token_mint.key(),
        ctx.accounts.moderator.key(),
        immediate
    );
    emit!(MissionxBanFailed {
        id: ctx.accounts.token_mint.key(),
        moderator: ctx.accounts.moderator.key(),
        immediate
    });

    Ok(())
}