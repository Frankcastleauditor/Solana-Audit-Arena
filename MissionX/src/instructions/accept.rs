use anchor_lang::prelude::*;
use anchor_spl::token_interface;

use crate::{constants::{CONFIG_PDA_SEED, MISSIONX_STATE}, error::MissionxErrors, events::MissionxAccepted, instructions::{ensure_missionx_state, ensure_enabled}, state::{missionx::{Missionx, MissionxStatus, MissionxTradeStatus}, global_config::Configuration}};


#[derive(Accounts)]
pub struct AcceptAccounts<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

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
        address = missionx_state.creator
    )]
    pub missionx_creator: AccountInfo<'info>,

    #[account(
        address = missionx_state.token_mint
    )]
    pub token_mint: InterfaceAccount<'info, token_interface::Mint>,

    pub system_program: Program<'info, System>,
}

pub fn accept_missionx(
    ctx: Context<AcceptAccounts>,
) -> Result<()> {
    ensure_enabled(&ctx.accounts.config)?;
    ensure_missionx_state(&ctx.accounts.missionx_state, MissionxStatus::Open)?;
    
    let clock = Clock::get()?;
    let missionx_state = &mut ctx.accounts.missionx_state;
    require!(clock.unix_timestamp as u64 <= missionx_state.open_timestamp + missionx_state.open_duration, MissionxErrors::MissionxFailedByExpiration);

    missionx_state.missionx_status = MissionxStatus::Accepted;
    missionx_state.trade_status = MissionxTradeStatus::Open;
    missionx_state.submitters[0] = Some(ctx.accounts.player.key());

    msg!(
        "MissionxAccepted; ID: {}, player: {}",
        ctx.accounts.token_mint.key(),
        ctx.accounts.player.key(),
    );
    emit!(MissionxAccepted {
        id: ctx.accounts.token_mint.key(),
        player: ctx.accounts.player.key(),
    });

    Ok(())
}

pub fn accept_missionx_multi(
    ctx: Context<AcceptAccounts>,
) -> Result<()> {
    ensure_enabled(&ctx.accounts.config)?;
    ensure_missionx_state(&ctx.accounts.missionx_state, MissionxStatus::Accepted)?;
    
    let missionx_state = &mut ctx.accounts.missionx_state;

    if missionx_state.submitters[1].is_none() {
        missionx_state.submitters[1] = Some(ctx.accounts.player.key());

    } else if missionx_state.submitters[2].is_none() {
        missionx_state.submitters[2] = Some(ctx.accounts.player.key());
    } else {
        return Err(MissionxErrors::TooManyPlayers.into());
    }

    msg!(
        "MissionxAccepted; ID: {}, player: {}",
        ctx.accounts.token_mint.key(),
        ctx.accounts.player.key(),
    );
    emit!(MissionxAccepted {
        id: ctx.accounts.token_mint.key(),
        player: ctx.accounts.player.key(),
    });

    Ok(())
}