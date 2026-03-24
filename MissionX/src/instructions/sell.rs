use anchor_lang::prelude::*;
use anchor_spl::{token_2022, token_interface};

use crate::{constants::{BPS, CONFIG_PDA_SEED, MISSIONX_STATE, MISSIONX_TOKEN_VAULT}, error::MissionxErrors, events::MissionxTrade, state::{missionx::Missionx, global_config::Configuration}, utils:: get_amount_out_sol};

use super::{ensure_missionx_tradable, ensure_enabled};

#[derive(Accounts)]
pub struct SellAccounts<'info> {
    #[account(mut)]
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

     /// CHECK: not required
     #[account(
        mut,
        address = config.fee_recipient
    )]
    pub fee_recipient: AccountInfo<'info>,

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
        associated_token::authority = user,
        associated_token::mint = token_mint,
        associated_token::token_program = token_program
    )]
    pub user_ata: InterfaceAccount<'info, token_interface::TokenAccount>,
}

pub fn sell(
    ctx: Context<SellAccounts>,
    sell_amount: u64,
    min_out: u64,
) -> Result<()> {
    ensure_enabled(&ctx.accounts.config)?;
    ensure_missionx_tradable(&ctx.accounts.missionx_state, &ctx.accounts.config, true)?;

    let missionx_state = &mut ctx.accounts.missionx_state;
    let sol_out = get_amount_out_sol(sell_amount, missionx_state.get_full_sol_reserve(), missionx_state.get_full_token_reserve())?;

    require!(sol_out >= min_out, MissionxErrors::SlippageLimit);
    
    let trading_fee = 
        sol_out.checked_mul(ctx.accounts.config.trade_fee_bps).ok_or(MissionxErrors::MathOverflow)? / BPS;

    missionx_state.reserve0 -= sol_out;
    missionx_state.reserve1 += sell_amount;

    token_2022::transfer_checked(
        CpiContext {
            program: ctx.accounts.token_program.to_account_info(),
            accounts: token_2022::TransferChecked {
                from: ctx.accounts.user_ata.to_account_info(), 
                mint: ctx.accounts.token_mint.to_account_info(), 
                to: ctx.accounts.token_vault_pda.to_account_info(), 
                authority: ctx.accounts.user.to_account_info()
            },
            remaining_accounts: vec![], 
            signer_seeds: &[]
        }, 
        sell_amount, 
        9
    )?;

    missionx_state.sub_lamports(sol_out)?;
    ctx.accounts.user.add_lamports(sol_out - trading_fee)?;
    ctx.accounts.fee_recipient.add_lamports(trading_fee)?;

    msg!(
        "MissionxTrade; ID: {}, user: {}, sell, sol:{}, tkn: {}, fee: {}",
        ctx.accounts.token_mint.key(),
        ctx.accounts.user.key(),
        sol_out,
        sell_amount,
        trading_fee
    );
    emit!(MissionxTrade {
        id: ctx.accounts.token_mint.key(),
        user: ctx.accounts.user.key(),
        sol_amount: sol_out,
        tkn_amount: sell_amount,
        fee: trading_fee,
        is_buy: false,
    });

    Ok(())
}