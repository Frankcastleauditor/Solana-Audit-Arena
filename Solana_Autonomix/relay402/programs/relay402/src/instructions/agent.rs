use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

use crate::constants::*;
use crate::errors::Relay402Error;
use crate::events::*;
use crate::state::{Agent, Config};
use crate::utils::{validate_endpoint, validate_metadata_uri};

fn emit_agent_updated(agent_key: Pubkey, agent: &Agent) {
    emit!(AgentUpdated {
        agent: agent_key,
        operator: agent.operator,
        payout: agent.payout,
        price: agent.price,
        endpoint: agent.endpoint.clone(),
        metadata_uri: agent.metadata_uri.clone(),
        metadata_hash: agent.metadata_hash,
        active: agent.active,
    });
}

// ------------------------------------------------------------------
// register_agent
// ------------------------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RegisterAgentArgs {
    pub endpoint: String,
    pub metadata_uri: String,
    pub metadata_hash: [u8; 32],
    pub price: u64,
    pub operator: Pubkey,
}

#[derive(Accounts)]
pub struct RegisterAgent<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    /// Id comes from the global counter, so the address is fixed by the
    /// program and ids are never reused (even after close).
    #[account(
        init,
        payer = owner,
        space = 8 + Agent::INIT_SPACE,
        seeds = [AGENT_SEED, config.next_agent_id.to_le_bytes().as_ref()],
        bump
    )]
    pub agent: Account<'info, Agent>,

    #[account(
        token::mint = config.mint,
        constraint = payout.key() != config.vault @ Relay402Error::InvalidPayout
    )]
    pub payout: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
}

pub fn register_agent_handler(ctx: Context<RegisterAgent>, args: RegisterAgentArgs) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(!config.paused, Relay402Error::Paused);
    validate_endpoint(&args.endpoint)?;
    validate_metadata_uri(&args.metadata_uri)?;
    require!(args.price >= config.min_price, Relay402Error::PriceBelowMinimum);
    require!(args.operator != Pubkey::default(), Relay402Error::InvalidOperator);

    let id = config.next_agent_id;
    config.next_agent_id = id.checked_add(1).ok_or(Relay402Error::MathOverflow)?;

    let agent = &mut ctx.accounts.agent;
    agent.id = id;
    agent.owner = ctx.accounts.owner.key();
    agent.pending_owner = None;
    agent.operator = args.operator;
    agent.payout = ctx.accounts.payout.key();
    agent.price = args.price;
    agent.endpoint = args.endpoint;
    agent.metadata_uri = args.metadata_uri;
    agent.metadata_hash = args.metadata_hash;
    agent.active = true;
    agent.pending_receipts = 0;
    agent.settled_count = 0;
    agent.feedback_count = 0;
    agent.score_sum = 0;
    agent.created_at = Clock::get()?.unix_timestamp;
    agent.bump = ctx.bumps.agent;

    emit!(AgentRegistered {
        agent: agent.key(),
        id,
        owner: agent.owner,
        operator: agent.operator,
        payout: agent.payout,
        price: agent.price,
        endpoint: agent.endpoint.clone(),
        metadata_uri: agent.metadata_uri.clone(),
        metadata_hash: agent.metadata_hash,
    });
    Ok(())
}

// ------------------------------------------------------------------
// update_agent
// ------------------------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateAgentArgs {
    pub endpoint: Option<String>,
    pub metadata_uri: Option<String>,
    pub metadata_hash: Option<[u8; 32]>,
    pub price: Option<u64>,
    pub operator: Option<Pubkey>,
}

#[derive(Accounts)]
pub struct UpdateAgent<'info> {
    pub owner: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        seeds = [AGENT_SEED, agent.id.to_le_bytes().as_ref()],
        bump = agent.bump,
        has_one = owner @ Relay402Error::Unauthorized
    )]
    pub agent: Account<'info, Agent>,
}

pub fn update_agent_handler(ctx: Context<UpdateAgent>, args: UpdateAgentArgs) -> Result<()> {
    let config = &ctx.accounts.config;
    let agent = &mut ctx.accounts.agent;

    if let Some(endpoint) = args.endpoint {
        validate_endpoint(&endpoint)?;
        agent.endpoint = endpoint;
    }
    if let Some(uri) = args.metadata_uri {
        validate_metadata_uri(&uri)?;
        agent.metadata_uri = uri;
    }
    if let Some(hash) = args.metadata_hash {
        agent.metadata_hash = hash;
    }
    // Price changes never touch pending receipts: each receipt stores the
    // amount the client actually paid.
    if let Some(price) = args.price {
        require!(price >= config.min_price, Relay402Error::PriceBelowMinimum);
        agent.price = price;
    }
    if let Some(operator) = args.operator {
        require!(operator != Pubkey::default(), Relay402Error::InvalidOperator);
        agent.operator = operator;
    }

    emit_agent_updated(agent.key(), agent);
    Ok(())
}

// ------------------------------------------------------------------
// set_payout
// ------------------------------------------------------------------

#[derive(Accounts)]
pub struct SetPayout<'info> {
    pub owner: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        seeds = [AGENT_SEED, agent.id.to_le_bytes().as_ref()],
        bump = agent.bump,
        has_one = owner @ Relay402Error::Unauthorized
    )]
    pub agent: Account<'info, Agent>,

    #[account(
        token::mint = config.mint,
        constraint = payout.key() != config.vault @ Relay402Error::InvalidPayout
    )]
    pub payout: Account<'info, TokenAccount>,
}

pub fn set_payout_handler(ctx: Context<SetPayout>) -> Result<()> {
    let agent = &mut ctx.accounts.agent;
    agent.payout = ctx.accounts.payout.key();
    emit_agent_updated(agent.key(), agent);
    Ok(())
}

// ------------------------------------------------------------------
// set_agent_active
// ------------------------------------------------------------------

#[derive(Accounts)]
pub struct SetAgentActive<'info> {
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [AGENT_SEED, agent.id.to_le_bytes().as_ref()],
        bump = agent.bump,
        has_one = owner @ Relay402Error::Unauthorized
    )]
    pub agent: Account<'info, Agent>,
}

pub fn set_agent_active_handler(ctx: Context<SetAgentActive>, active: bool) -> Result<()> {
    let agent = &mut ctx.accounts.agent;
    agent.active = active;
    emit_agent_updated(agent.key(), agent);
    Ok(())
}

// ------------------------------------------------------------------
// transfer_agent / accept_agent (two-step ownership transfer)
// ------------------------------------------------------------------

#[derive(Accounts)]
pub struct TransferAgent<'info> {
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [AGENT_SEED, agent.id.to_le_bytes().as_ref()],
        bump = agent.bump,
        has_one = owner @ Relay402Error::Unauthorized
    )]
    pub agent: Account<'info, Agent>,
}

pub fn transfer_agent_handler(ctx: Context<TransferAgent>, new_owner: Pubkey) -> Result<()> {
    require!(new_owner != Pubkey::default(), Relay402Error::InvalidAuthority);
    let agent = &mut ctx.accounts.agent;
    agent.pending_owner = Some(new_owner);

    emit!(AgentOwnershipProposed {
        agent: agent.key(),
        owner: agent.owner,
        pending_owner: new_owner,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct AcceptAgent<'info> {
    pub new_owner: Signer<'info>,

    #[account(
        mut,
        seeds = [AGENT_SEED, agent.id.to_le_bytes().as_ref()],
        bump = agent.bump,
        constraint = agent.pending_owner == Some(new_owner.key())
            @ Relay402Error::NoPendingTransfer
    )]
    pub agent: Account<'info, Agent>,
}

pub fn accept_agent_handler(ctx: Context<AcceptAgent>) -> Result<()> {
    let agent = &mut ctx.accounts.agent;
    let old_owner = agent.owner;
    agent.owner = ctx.accounts.new_owner.key();
    agent.pending_owner = None;

    // Operator and payout are NOT changed here. The new owner is expected to
    // rotate them with update_agent / set_payout. See README trust model.
    emit!(AgentOwnershipTransferred {
        agent: agent.key(),
        old_owner,
        new_owner: agent.owner,
    });
    Ok(())
}

// ------------------------------------------------------------------
// close_agent
// ------------------------------------------------------------------

#[derive(Accounts)]
pub struct CloseAgent<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    /// Requires no pending receipts: every Pending receipt needs this account
    /// to settle or refund, so closing early would lock client funds.
    #[account(
        mut,
        seeds = [AGENT_SEED, agent.id.to_le_bytes().as_ref()],
        bump = agent.bump,
        has_one = owner @ Relay402Error::Unauthorized,
        constraint = !agent.active && agent.pending_receipts == 0
            @ Relay402Error::AgentNotClosable,
        close = owner
    )]
    pub agent: Account<'info, Agent>,
}

pub fn close_agent_handler(ctx: Context<CloseAgent>) -> Result<()> {
    emit!(AgentClosed {
        agent: ctx.accounts.agent.key(),
        id: ctx.accounts.agent.id,
    });
    Ok(())
}
