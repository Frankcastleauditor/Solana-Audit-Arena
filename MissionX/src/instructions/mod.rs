pub mod accept;
pub mod buy;
pub mod complete;
pub mod create_missionx;
pub mod init;
pub mod migrate;
pub mod sell;
pub mod set_global_config;
pub mod withdraw;
pub mod moderate;
pub mod update_moderator;
pub mod fail_missionx;


pub use accept::*;
pub use buy::*;
pub use complete::*;
pub use create_missionx::*;
pub use init::*;
pub use migrate::*;
pub use sell::*;
pub use set_global_config::*;
pub use withdraw::*;
pub use moderate::*;
pub use update_moderator::*;
pub use fail_missionx::*;

use anchor_lang::prelude::*;
use anchor_spl::{token_interface, token_2022};
use crate::{constants::MISSIONX_STATE, error::MissionxErrors, state::{missionx::{Missionx, MissionxStatus, MissionxTradeStatus}, global_config::Configuration, moderator::ModeratorState}};

pub fn ensure_enabled<'info>(config: &Box<Account<'info, Configuration>>) -> Result<()> {
    require!(config.initialized, MissionxErrors::NotInitialized);
    require!(config.is_enabled, MissionxErrors::NotEnabled);

    Ok(())
}

pub fn ensure_missionx_state<'info>(missionx_state: &Box<Account<'info, Missionx>>, expected_state: MissionxStatus) -> Result<()> {
    require!(!missionx_state.is_blocked, MissionxErrors::MissionxBlocked);
    require!(missionx_state.missionx_status == expected_state, MissionxErrors::WrongMissionxSatus);

    Ok(())
}

pub fn ensure_missionx_tradable<'info>(missionx_state: &Box<Account<'info, Missionx>>, config: &Box<Account<'info, Configuration>>, is_sell: bool) -> Result<()> {
    if !is_sell {
        require!(!missionx_state.is_blocked, MissionxErrors::MissionxBlocked);
    }
    require!(missionx_state.trade_status == MissionxTradeStatus::Open, MissionxErrors::MissionxNotTradable);
    
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp as u64;

    if let Some(fail_ts) = missionx_state.fail_ts {
        require!(current_time <= (fail_ts + config.fail_grace_period), MissionxErrors::FailedMissionxTradeGracePeriodExpired);
    } else if missionx_state.missionx_status == MissionxStatus::Open {
        let fail_ts = missionx_state.open_timestamp + missionx_state.open_duration;
        require!(current_time <= (fail_ts + config.fail_grace_period), MissionxErrors::FailedMissionxTradeGracePeriodExpired);
    }    

    Ok(())
}

pub fn ensure_moderator_enabled(moderator_config: &ModeratorState) -> Result<()> {
    require!(moderator_config.is_enabled, MissionxErrors::ModeratorIsDisabled); 

    Ok(())
}

pub fn is_migration_status(status: MissionxTradeStatus) -> bool {
    match status {
        MissionxTradeStatus::MigrationRequired => true,
        MissionxTradeStatus::Migrated => true,
        _ => false
    }
}

pub fn missionx_reached_migration<'info>(missionx_state: &Box<Account<'info, Missionx>>) -> bool {
    is_migration_status(missionx_state.trade_status)
}

pub fn missionx_has_failed<'info>(missionx_state: &Box<Account<'info, Missionx>>) -> Result<bool> {
    match missionx_state.missionx_status {
        MissionxStatus::Unverified => Ok(false),
        MissionxStatus::Censored =>   Ok(false),
        MissionxStatus::Accepted =>   Ok(false),
        MissionxStatus::Completed =>  Ok(false),
        MissionxStatus::Failed =>     Ok(true),
        MissionxStatus::Open => {           
                let clock = Clock::get()?;
                let current_time = clock.unix_timestamp as u64;

                Ok(current_time > (missionx_state.open_timestamp + missionx_state.open_duration))
            },
        MissionxStatus::Withdrawn => unreachable!(),
    }
}

pub fn do_token_payout<'info>(
    missionx_state: &mut Box<Account<'info, Missionx>>,
    token_program: &AccountInfo<'info>,
    token_mint: &InterfaceAccount<'info, token_interface::Mint>,
    token_vault_pda: &InterfaceAccount<'info, token_interface::TokenAccount>,
    player_ata: Option<&InterfaceAccount<'info, token_interface::TokenAccount>>,
    fee_rcp_ata: Option<&InterfaceAccount<'info, token_interface::TokenAccount>>,
    creator_ata: &InterfaceAccount<'info, token_interface::TokenAccount>,
    missionx_state_bump: u8
) -> Result<()> {
    if missionx_reached_migration(missionx_state) {
        if (missionx_state.missionx_status == MissionxStatus::Completed) && (missionx_state.token_player_payout > 0) {
            token_2022::transfer_checked(
                CpiContext {
                    program: token_program.to_account_info(),
                    accounts: token_2022::TransferChecked { 
                        mint: token_mint.to_account_info(),
                        authority: missionx_state.to_account_info(),
                        from: token_vault_pda.to_account_info(),
                        to: player_ata.unwrap().to_account_info()
                    },
                    remaining_accounts: vec![],
                    signer_seeds: &[&[
                        MISSIONX_STATE,
                        &token_mint.key().to_bytes(),
                        &[missionx_state_bump]
                    ]]
                },
                missionx_state.token_player_payout,
                9
            )?;

            missionx_state.token_player_payout = 0;
        } else if missionx_has_failed(missionx_state)? {
            token_2022::transfer_checked(
                CpiContext {
                    program: token_program.to_account_info(),
                    accounts: token_2022::TransferChecked { 
                        mint: token_mint.to_account_info(),
                        authority: missionx_state.to_account_info(),
                        from: token_vault_pda.to_account_info(),
                        to: fee_rcp_ata.unwrap().to_account_info()
                    },
                    remaining_accounts: vec![],
                    signer_seeds: &[&[
                        MISSIONX_STATE,
                        &token_mint.key().to_bytes(),
                        &[missionx_state_bump]
                    ]]
                },
                missionx_state.token_player_payout,
                9
            )?;

            missionx_state.token_player_payout = 0;

        }

        if missionx_state.token_creator_payout > 0 {
            token_2022::transfer_checked(
                CpiContext {
                    program: token_program.to_account_info(),
                    accounts: token_2022::TransferChecked { 
                        mint: token_mint.to_account_info(),
                        authority: missionx_state.to_account_info(),
                        from: token_vault_pda.to_account_info(),
                        to: creator_ata.to_account_info()
                    },
                    remaining_accounts: vec![],
                    signer_seeds: &[&[
                        MISSIONX_STATE,
                        &token_mint.key().to_bytes(),
                        &[missionx_state_bump]
                    ]]
                },
                missionx_state.token_creator_payout,
                9
            )?;
            missionx_state.token_creator_payout = 0;
        }

    }

    Ok(())
}


pub fn refund_payout<'info>(
    missionx_state: &Box<Account<'info, Missionx>>,
    config: &Box<Account<'info, Configuration>>,
    fee_recipient: &AccountInfo<'info>,
    missionx_creator: &AccountInfo<'info>,
) -> Result<()>{
    let payout = missionx_state.payout_amount;
    missionx_state.sub_lamports(payout)?;
    fee_recipient.add_lamports(config.fail_fee)?;
    missionx_creator.add_lamports(
        payout.checked_sub(config.fail_fee).ok_or(MissionxErrors::MathOverflow)?
    )?;

    Ok(())
}