use anchor_lang::prelude::*;
use anchor_spl::{token_2022, token_interface};

use crate::{constants::{CONFIG_PDA_SEED, MISSIONX_STATE, MISSIONX_TOKEN_VAULT}, error::MissionxErrors, events::MissionxWithdraw, state::{missionx::{Missionx, MissionxStatus, MissionxTradeStatus}, global_config::Configuration}};


#[derive(Accounts)]
pub struct WithdrawAccounts<'info> {
    #[account(
        mut,
        address = config.owner
    )]
    pub owner: Signer<'info>,

    #[account(
        seeds = [CONFIG_PDA_SEED],
        bump
    )]
    pub config: Box<Account<'info, Configuration>>,

    /// CHECK: not required
    #[account(
        mut,
        address = missionx_state.token_mint
    )]
    pub token_mint: InterfaceAccount<'info, token_interface::Mint>,

    #[account(
        mut,
        seeds = [MISSIONX_STATE, &token_mint.key().to_bytes()],
        bump
    )]
    pub missionx_state: Box<Account<'info, Missionx>>,

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
        token::authority = owner,
        token::mint = token_mint,
        token::token_program = token_program
    )]
    pub owner_ata: InterfaceAccount<'info, token_interface::TokenAccount>,

    /// CHECK: not required
    #[account(
        executable,
        address = missionx_state.token_program
    )]
    pub token_program: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn withdraw_from_missionx(
    ctx: Context<WithdrawAccounts>,
) -> Result<()> {
    let clock = Clock::get()?;
    let allowed_to_withdraw = match ctx.accounts.missionx_state.missionx_status {
        MissionxStatus::Open => {
            let fail_ts = ctx.accounts.missionx_state.open_timestamp + ctx.accounts.missionx_state.open_duration;
            let fail_graced = fail_ts + ctx.accounts.config.fail_grace_period;

            fail_graced < (clock.unix_timestamp as u64)
        },
        MissionxStatus::Failed => {
            let fail_graced = ctx.accounts.missionx_state.fail_ts.unwrap() + ctx.accounts.config.fail_grace_period;

            fail_graced < (clock.unix_timestamp as u64)
        },
        _ => false
    };
    
    require!(allowed_to_withdraw, MissionxErrors::WithdrawIsNotAllowed);
    
    let missionx_state = &mut ctx.accounts.missionx_state;
    missionx_state.missionx_status = MissionxStatus::Withdrawn;
    missionx_state.trade_status = MissionxTradeStatus::Withdrawn;
    missionx_state.old_trade_status = None;

    let mut balance = ctx.accounts.token_vault_pda.amount;

    token_2022::transfer_checked(
        CpiContext {
            program: ctx.accounts.token_program.to_account_info(),
            accounts: token_2022::TransferChecked {
                from: ctx.accounts.token_vault_pda.to_account_info(), 
                mint: ctx.accounts.token_mint.to_account_info(), 
                to: ctx.accounts.owner_ata.to_account_info(), 
                authority: missionx_state.to_account_info()
            },
            remaining_accounts: vec![], 
            signer_seeds: &[&[
                    MISSIONX_STATE,
                    &ctx.accounts.token_mint.key().to_bytes(),
                    &[ctx.bumps.missionx_state]
                ]]
        },
        balance, 
        9
    )?;

    balance = missionx_state.to_account_info().lamports();
    let rent_exempt = ctx.accounts.rent.minimum_balance(missionx_state.to_account_info().data_len()).max(1);
    balance = balance.checked_sub(rent_exempt).unwrap_or(0);
    missionx_state.sub_lamports(balance)?;
    ctx.accounts.owner.add_lamports(balance)?;

    msg!("MissionxWithdraw; ID: {}",
        ctx.accounts.token_mint.key(),
    );
    emit!(MissionxWithdraw {
        id: ctx.accounts.token_mint.key(),
    });

    Ok(())
}