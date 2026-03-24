use anchor_lang::prelude::*;
use anchor_spl::{token_2022, token_interface};

use crate::{
    constants::{CONFIG_PDA_SEED, MISSIONX_STATE, MISSIONX_TOKEN_VAULT},
    error::MissionxErrors,
    events::MissionxMigrated,
    instructions::{do_token_payout, ensure_enabled},
    state::{
        global_config::Configuration,
        missionx::{Missionx, MissionxStatus, MissionxTradeStatus},
    },
};

#[derive(Accounts)]
pub struct MigrateAccounts<'info> {
    #[account(
        mut,
        address = config.executor
    )]
    pub executor: Signer<'info>,

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
    #[account(mut)]
    pub player: AccountInfo<'info>,

    /// CHECK: not required
    #[account(
        mut,
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
        token::authority = executor,
        token::mint = token_mint,
        token::token_program = token_program
    )]
    pub executor_ata: InterfaceAccount<'info, token_interface::TokenAccount>,

    #[account(
        mut,
        associated_token::authority = missionx_state.creator,
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

    #[account(
        mut,
        associated_token::authority = fee_recipient,
        associated_token::mint = token_mint,
        associated_token::token_program = token_program
    )]
    pub fee_recipient_ata: InterfaceAccount<'info, token_interface::TokenAccount>,
}

pub fn migrate(ctx: Context<MigrateAccounts>) -> Result<()> {
    ensure_enabled(&ctx.accounts.config)?;
    require!(
        !ctx.accounts.missionx_state.is_blocked,
        MissionxErrors::MissionxBlocked
    );
    require!(
        ctx.accounts.missionx_state.trade_status == MissionxTradeStatus::MigrationRequired,
        MissionxErrors::MissionxNotMigrationReady
    );
    let player_ata_account =
        if ctx.accounts.missionx_state.missionx_status == MissionxStatus::Completed {
            if let Some(player) = ctx.accounts.missionx_state.submitters[0] {
                require!(
                    player == ctx.accounts.player.key(),
                    MissionxErrors::IncorrectMissionxPlayerAccount
                );
                Some(&ctx.accounts.player_ata)
            } else {
                None
            }
        } else {
            None
        };

    let missionx_state = &mut ctx.accounts.missionx_state;
    let executor = &mut ctx.accounts.executor;
    let fee_recipient = &mut ctx.accounts.fee_recipient;

    token_2022::transfer_checked(
        CpiContext {
            program: ctx.accounts.token_program.to_account_info(),
            accounts: token_2022::TransferChecked {
                mint: ctx.accounts.token_mint.to_account_info(),
                authority: missionx_state.to_account_info(),
                from: ctx.accounts.token_vault_pda.to_account_info(),
                to: ctx.accounts.executor_ata.to_account_info(),
            },
            remaining_accounts: vec![],
            signer_seeds: &[&[
                MISSIONX_STATE,
                &ctx.accounts.token_mint.key().to_bytes(),
                &[ctx.bumps.missionx_state],
            ]],
        },
        missionx_state.reserve1,
        9,
    )?;

    do_token_payout(
        missionx_state,
        &ctx.accounts.token_program,
        &ctx.accounts.token_mint,
        &ctx.accounts.token_vault_pda,
        player_ata_account,
        Some(&ctx.accounts.fee_recipient_ata),
        &ctx.accounts.creator_ata,
        ctx.bumps.missionx_state,
    )?;

    missionx_state.trade_status = MissionxTradeStatus::Migrated;

    let reserve = missionx_state.reserve0;
    missionx_state.sub_lamports(reserve)?;
    fee_recipient.add_lamports(missionx_state.migration_fee)?;
    executor.add_lamports(
        reserve
            .checked_sub(missionx_state.migration_fee)
            .ok_or(MissionxErrors::MathOverflow)?,
    )?;

    msg!("MissionxMigrated; ID: {}", ctx.accounts.token_mint.key(),);
    emit!(MissionxMigrated {
        id: ctx.accounts.token_mint.key(),
    });

    Ok(())
}
