use std::ops::Deref;

use anchor_lang::prelude::*;
use anchor_spl::token_interface;

use crate::{constants::{CONFIG_PDA_SEED, MISSIONX_MODERATOR, MISSIONX_STATE, MISSIONX_TOKEN_VAULT}, error::MissionxErrors, events::{MissionxCompleted, MissionxPlayerFailed}, instructions::{do_token_payout, ensure_enabled, ensure_moderator_enabled}, state::{missionx::{Missionx, MissionxStatus}, global_config::Configuration, moderator::ModeratorState}};

use super::ensure_missionx_state;

#[derive(Accounts)]
pub struct ConfirmAccounts<'info> {
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
        address = missionx_state.creator
    )]
    pub missionx_creator: AccountInfo<'info>,

    /// CHECK: not required
    #[account(
        mut
    )]
    pub player: AccountInfo<'info>,

    /// CHECK: not required
    #[account(
        mut,
        address = config.fee_recipient
    )]
    pub fee_recipient: AccountInfo<'info>,

    #[account(
        seeds = [MISSIONX_MODERATOR, &moderator.key.to_bytes()],
        bump
    )]
    pub moderator_config: Option<Account<'info, ModeratorState>>,


    #[account(
        address = missionx_state.token_mint
    )]
    pub token_mint: InterfaceAccount<'info, token_interface::Mint>,

    /// CHECK: not required
    #[account(
        executable,
        address = missionx_state.token_program
    )]
    pub token_program: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [MISSIONX_TOKEN_VAULT, &token_mint.key().to_bytes(), &missionx_state.key().to_bytes()],
        bump,
        token::authority = missionx_state,
        token::mint = token_mint,
        token::token_program = token_program
    )]
    pub token_vault_pda: InterfaceAccount<'info, token_interface::TokenAccount>,

    #[account(
        mut,
        associated_token::authority = missionx_creator,
        associated_token::mint = token_mint,
        associated_token::token_program = token_program
    )]
    pub creator_ata: InterfaceAccount<'info, token_interface::TokenAccount>,

    #[account(
        mut,
        associated_token::authority = player,
        associated_token::mint = token_mint,
        associated_token::token_program = token_program
    )]
    pub player_ata: InterfaceAccount<'info, token_interface::TokenAccount>,

    pub system_program: Program<'info, System>,
}

pub fn complete_missionx(
    ctx: Context<ConfirmAccounts>,
    is_successful: bool,
) -> Result<()> {
    ensure_enabled(&ctx.accounts.config)?;
    ensure_missionx_state(&ctx.accounts.missionx_state, MissionxStatus::Accepted)?;

    require!(ctx.accounts.missionx_state.submitters.iter().any(|e| e.is_some_and(|ev| ev == ctx.accounts.player.key())), MissionxErrors::IncorrectMissionxPlayerAccount);
    let clock = Clock::get()?;

    if ctx.accounts.moderator.key() != ctx.accounts.missionx_creator.key() {
        let config: &Account<'_, ModeratorState> = ctx.accounts.moderator_config.as_ref().unwrap();
        ensure_moderator_enabled(config.deref())?;
    }

    if is_successful {
        let missionx_state = &mut ctx.accounts.missionx_state;
        missionx_state.missionx_status = MissionxStatus::Completed;
        missionx_state.submitters[0] = Some(ctx.accounts.player.key());
        missionx_state.submitters[1] = None;
        missionx_state.submitters[2] = None;
        missionx_state.success_time = clock.unix_timestamp as u64;

        do_token_payout(
            missionx_state, 
            &ctx.accounts.token_program, 
            &ctx.accounts.token_mint, 
            &ctx.accounts.token_vault_pda, 
            Some(&ctx.accounts.player_ata),
            None,
            &ctx.accounts.creator_ata,
            ctx.bumps.missionx_state
        )?;

        missionx_state.sub_lamports(missionx_state.payout_amount)?;
        ctx.accounts.player.add_lamports(missionx_state.payout_amount)?;

        msg!(
            "MissionxCompleted; ID: {}, player: {}",
            ctx.accounts.token_mint.key(),
            ctx.accounts.player.key(),
        );
        emit!(MissionxCompleted {
            id: ctx.accounts.token_mint.key(),
            player: ctx.accounts.player.key(),
        });

    } else {
        let missionx_state = &mut ctx.accounts.missionx_state;

        let submitters_last_idx = if missionx_state.submitters[2].is_some() {2}
            else if missionx_state.submitters[1].is_some() {1}
            else if missionx_state.submitters[0].is_some() {0}
            else { unreachable!() };
        let submitters_idx = missionx_state.submitters.iter().enumerate().find_map(
            |(i,k)| {if k.is_some_and(|kv| kv == ctx.accounts.player.key()) {Some(i)} else {None}}
        ).unwrap();

        missionx_state.submitters[submitters_idx] = missionx_state.submitters[submitters_last_idx];
        missionx_state.submitters[submitters_last_idx] = None;


        if submitters_last_idx == 0 {
            missionx_state.missionx_status = MissionxStatus::Open;
            missionx_state.open_timestamp = clock.unix_timestamp as u64;
        }
        
        msg!(
            "MissionxPlayerFailed; ID: {}, player: {}, state: {:?}",
            ctx.accounts.token_mint.key(),
            ctx.accounts.player.key(),
            missionx_state.missionx_status,
        );
        emit!(MissionxPlayerFailed {
            id: ctx.accounts.token_mint.key(),
            player: ctx.accounts.player.key(),
            missionx_state: missionx_state.missionx_status,
        });
    };
  
    
    Ok(())
}