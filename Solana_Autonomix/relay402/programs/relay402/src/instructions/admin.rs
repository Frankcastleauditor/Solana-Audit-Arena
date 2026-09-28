use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::Relay402Error;
use crate::events::*;
use crate::program::Relay402;
use crate::state::Config;

// ------------------------------------------------------------------
// initialize_config
// ------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + Config::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,

    /// Payment mint. `Account<Mint>` enforces the classic SPL Token owner, so
    /// Token-2022 mints (and their extensions) are rejected here.
    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = admin,
        seeds = [VAULT_SEED],
        bump,
        token::mint = mint,
        token::authority = config
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(token::mint = mint)]
    pub treasury: Account<'info, TokenAccount>,

    /// Only the upgrade authority can initialize. Stops anyone from
    /// front-running the one-time init with their own mint and admin.
    #[account(
        constraint = program.programdata_address()? == Some(program_data.key())
            @ Relay402Error::Unauthorized
    )]
    pub program: Program<'info, Relay402>,

    #[account(
        constraint = program_data.upgrade_authority_address == Some(admin.key())
            @ Relay402Error::Unauthorized
    )]
    pub program_data: Account<'info, ProgramData>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_config_handler(
    ctx: Context<InitializeConfig>,
    fee_bps: u16,
    min_price: u64,
) -> Result<()> {
    require!(fee_bps <= MAX_FEE_BPS, Relay402Error::FeeTooHigh);
    require!(min_price > 0, Relay402Error::InvalidMinPrice);

    let config = &mut ctx.accounts.config;
    config.admin = ctx.accounts.admin.key();
    config.pending_admin = None;
    config.mint = ctx.accounts.mint.key();
    config.treasury = ctx.accounts.treasury.key();
    config.vault = ctx.accounts.vault.key();
    config.fee_bps = fee_bps;
    config.min_price = min_price;
    config.next_agent_id = 0;
    config.paused = false;
    config.bump = ctx.bumps.config;
    config.vault_bump = ctx.bumps.vault;

    emit!(ConfigInitialized {
        admin: config.admin,
        mint: config.mint,
        treasury: config.treasury,
        fee_bps,
        min_price,
    });
    Ok(())
}

// ------------------------------------------------------------------
// update_config
// ------------------------------------------------------------------

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ Relay402Error::Unauthorized
    )]
    pub config: Account<'info, Config>,
}

pub fn update_config_handler(
    ctx: Context<UpdateConfig>,
    fee_bps: Option<u16>,
    min_price: Option<u64>,
    paused: Option<bool>,
) -> Result<()> {
    let config = &mut ctx.accounts.config;

    // A new fee only applies to payments created after this point. Pending
    // receipts carry their own fee snapshot.
    if let Some(fee_bps) = fee_bps {
        require!(fee_bps <= MAX_FEE_BPS, Relay402Error::FeeTooHigh);
        config.fee_bps = fee_bps;
    }
    if let Some(min_price) = min_price {
        require!(min_price > 0, Relay402Error::InvalidMinPrice);
        config.min_price = min_price;
    }
    if let Some(paused) = paused {
        config.paused = paused;
    }

    emit!(ConfigUpdated {
        fee_bps: config.fee_bps,
        min_price: config.min_price,
        paused: config.paused,
        treasury: config.treasury,
    });
    Ok(())
}

// ------------------------------------------------------------------
// set_treasury
// ------------------------------------------------------------------

#[derive(Accounts)]
pub struct SetTreasury<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ Relay402Error::Unauthorized
    )]
    pub config: Account<'info, Config>,

    /// Must hold the payment mint and must not be the escrow vault (fees
    /// sent to the vault would be mixed with client escrow).
    #[account(
        token::mint = config.mint,
        constraint = treasury.key() != config.vault @ Relay402Error::InvalidTreasury
    )]
    pub treasury: Account<'info, TokenAccount>,
}

pub fn set_treasury_handler(ctx: Context<SetTreasury>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.treasury = ctx.accounts.treasury.key();

    emit!(ConfigUpdated {
        fee_bps: config.fee_bps,
        min_price: config.min_price,
        paused: config.paused,
        treasury: config.treasury,
    });
    Ok(())
}

// ------------------------------------------------------------------
// propose_admin / accept_admin (two-step rotation)
// ------------------------------------------------------------------

#[derive(Accounts)]
pub struct ProposeAdmin<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ Relay402Error::Unauthorized
    )]
    pub config: Account<'info, Config>,
}

pub fn propose_admin_handler(ctx: Context<ProposeAdmin>, new_admin: Pubkey) -> Result<()> {
    require!(new_admin != Pubkey::default(), Relay402Error::InvalidAuthority);
    let config = &mut ctx.accounts.config;
    config.pending_admin = Some(new_admin);

    emit!(AdminTransferProposed {
        admin: config.admin,
        pending_admin: new_admin,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct AcceptAdmin<'info> {
    pub new_admin: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        constraint = config.pending_admin == Some(new_admin.key())
            @ Relay402Error::NoPendingTransfer
    )]
    pub config: Account<'info, Config>,
}

pub fn accept_admin_handler(ctx: Context<AcceptAdmin>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    let old_admin = config.admin;
    config.admin = ctx.accounts.new_admin.key();
    config.pending_admin = None;

    emit!(AdminTransferred {
        old_admin,
        new_admin: config.admin,
    });
    Ok(())
}
