use anchor_lang::{prelude::*, system_program};
use anchor_spl::{token_2022, token_interface};

use crate::{constants::{BPS, CONFIG_PDA_SEED, MISSIONX_STATE, MISSIONX_TOKEN_VAULT}, error::MissionxErrors, events::{MissionxMigrationRequired, MissionxTrade}, instructions::missionx_has_failed, state::{missionx::{Missionx, MissionxTradeStatus}, global_config::Configuration}, utils::{get_amount_in_sol, get_amount_out_tokens}};

use super::{ensure_missionx_tradable, ensure_enabled};

#[derive(Accounts)]
pub struct BuyAccounts<'info> {
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

    pub system_program: Program<'info, System>,
}

pub fn buy(
    ctx: Context<BuyAccounts>,
    buy_amount: u64,
    pay_cap: u64,
) -> Result<()> {
    ensure_enabled(&ctx.accounts.config)?;
    ensure_missionx_tradable(&ctx.accounts.missionx_state, &ctx.accounts.config, false)?;
    
    let missionx_state = &mut ctx.accounts.missionx_state;

    let mut effective_buy_amount = buy_amount;
    let mut effective_sol_spend = get_amount_in_sol(buy_amount, missionx_state.get_full_sol_reserve(), missionx_state.get_full_token_reserve())?;
    require!(effective_sol_spend <= pay_cap, MissionxErrors::SlippageLimit);

    if missionx_state.migration_threshold <= (missionx_state.reserve0 + effective_sol_spend) {
        effective_sol_spend = missionx_state.migration_threshold - missionx_state.reserve0;
        
        effective_buy_amount = if effective_sol_spend > 0 {
            get_amount_out_tokens(effective_sol_spend, missionx_state.get_full_sol_reserve(), missionx_state.get_full_token_reserve())?
        } else { 0 };

        if !missionx_has_failed(&missionx_state)? {
            missionx_state.trade_status = MissionxTradeStatus::MigrationRequired;

            msg!("MissionxMigrationReady; ID: {}", ctx.accounts.token_mint.key());
            emit!(MissionxMigrationRequired {
                id: ctx.accounts.token_mint.key()
            });
        }
    }

    let trading_fee = 
        effective_sol_spend.checked_mul(ctx.accounts.config.trade_fee_bps).ok_or(MissionxErrors::MathOverflow)? / BPS;
    
    if effective_sol_spend > 0 {

        missionx_state.reserve0 += effective_sol_spend;
        missionx_state.reserve1 -= effective_buy_amount;

        system_program::transfer(CpiContext::new(
                ctx.accounts.system_program.to_account_info(), 
                system_program::Transfer { 
                    from: ctx.accounts.user.to_account_info(),
                    to: ctx.accounts.missionx_state.to_account_info(),
                }
            ),
            effective_sol_spend
        )?;

        system_program::transfer(CpiContext::new(
                ctx.accounts.system_program.to_account_info(), 
                system_program::Transfer { 
                    from: ctx.accounts.user.to_account_info(),
                    to: ctx.accounts.fee_recipient.to_account_info(),
                }
            ),
            trading_fee
        )?;

        token_2022::transfer_checked(
            CpiContext {
                program: ctx.accounts.token_program.to_account_info(),
                accounts: token_2022::TransferChecked { 
                    from: ctx.accounts.token_vault_pda.to_account_info(), 
                    mint: ctx.accounts.token_mint.to_account_info(), 
                    to: ctx.accounts.user_ata.to_account_info(), 
                    authority: ctx.accounts.missionx_state.to_account_info()
                },
                remaining_accounts: vec![], 
                signer_seeds: &[&[
                    MISSIONX_STATE,
                    &ctx.accounts.token_mint.key().to_bytes(),
                    &[ctx.bumps.missionx_state]
                ]]
            },
            effective_buy_amount, 
            9
        )?;
    }
    
    msg!(
        "MissionxTrade; ID: {}, user: {}, buy, sol:{}, tkn: {}, fee: {}",
        ctx.accounts.token_mint.key(),
        ctx.accounts.user.key(),
        effective_sol_spend,
        effective_buy_amount,
        trading_fee
    );
    emit!(MissionxTrade {
        id: ctx.accounts.token_mint.key(),
        user: ctx.accounts.user.key(),
        sol_amount: effective_sol_spend,
        tkn_amount: effective_buy_amount,
        fee: trading_fee,
        is_buy: true,
    });

    Ok(())
}