use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

use crate::constants::*;
use crate::errors::Relay402Error;
use crate::events::*;
use crate::state::{Agent, Config, Receipt, ReceiptStatus};
use crate::utils::compute_fee;

/// Transfer out of the shared vault, signed by the config PDA.
fn transfer_from_vault<'info>(
    token_program: &Program<'info, Token>,
    vault: &Account<'info, TokenAccount>,
    mint: &Account<'info, Mint>,
    to: &Account<'info, TokenAccount>,
    config: &Account<'info, Config>,
    amount: u64,
) -> Result<()> {
    let signer_seeds: &[&[&[u8]]] = &[&[CONFIG_SEED, &[config.bump]]];
    token::transfer_checked(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            TransferChecked {
                from: vault.to_account_info(),
                mint: mint.to_account_info(),
                to: to.to_account_info(),
                authority: config.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        mint.decimals,
    )
}

// ------------------------------------------------------------------
// create_payment
// ------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct CreatePayment<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        seeds = [AGENT_SEED, agent.id.to_le_bytes().as_ref()],
        bump = agent.bump
    )]
    pub agent: Account<'info, Agent>,

    /// `init` fails if a live receipt already exists for this
    /// (agent, client, nonce), so a nonce cannot be reused while its receipt
    /// is open.
    #[account(
        init,
        payer = client,
        space = 8 + Receipt::INIT_SPACE,
        seeds = [
            RECEIPT_SEED,
            agent.key().as_ref(),
            client.key().as_ref(),
            nonce.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub receipt: Account<'info, Receipt>,

    #[account(address = config.mint)]
    pub mint: Account<'info, Mint>,

    #[account(mut, token::mint = mint, token::authority = client)]
    pub client_token: Account<'info, TokenAccount>,

    #[account(mut, address = config.vault)]
    pub vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn create_payment_handler(
    ctx: Context<CreatePayment>,
    nonce: u64,
    window_secs: i64,
    max_amount: u64,
) -> Result<()> {
    let config = &ctx.accounts.config;
    let agent = &mut ctx.accounts.agent;

    require!(!config.paused, Relay402Error::Paused);
    require!(agent.active, Relay402Error::AgentInactive);
    require!(
        (MIN_PAYMENT_WINDOW_SECS..=MAX_PAYMENT_WINDOW_SECS).contains(&window_secs),
        Relay402Error::InvalidPaymentWindow
    );

    let amount = agent.price;
    // Protects the client if the owner raises the price between the 402
    // quote and this transaction landing.
    require!(amount <= max_amount, Relay402Error::PriceAboveMax);
    // min_price can be raised after the agent set its price.
    require!(amount >= config.min_price, Relay402Error::PriceBelowMinimum);

    let now = Clock::get()?.unix_timestamp;
    let expires_at = now
        .checked_add(window_secs)
        .ok_or(Relay402Error::MathOverflow)?;

    let receipt = &mut ctx.accounts.receipt;
    receipt.agent = agent.key();
    receipt.client = ctx.accounts.client.key();
    receipt.nonce = nonce;
    receipt.amount = amount;
    receipt.fee_bps = config.fee_bps;
    receipt.created_at = now;
    receipt.expires_at = expires_at;
    receipt.settled_at = 0;
    receipt.status = ReceiptStatus::Pending;
    receipt.bump = ctx.bumps.receipt;

    agent.pending_receipts = agent
        .pending_receipts
        .checked_add(1)
        .ok_or(Relay402Error::MathOverflow)?;

    token::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.client_token.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.client.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    emit!(PaymentCreated {
        receipt: receipt.key(),
        agent: receipt.agent,
        client: receipt.client,
        nonce,
        amount,
        fee_bps: receipt.fee_bps,
        expires_at,
    });
    Ok(())
}

// ------------------------------------------------------------------
// settle_payment
// ------------------------------------------------------------------

#[derive(Accounts)]
pub struct SettlePayment<'info> {
    /// Agent's service key. Only the agent can claim a payment, which stops
    /// third parties (or the facilitator alone) from settling a receipt for
    /// work that was never done. Any account can pay the transaction fee.
    pub operator: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(mut, token::mint = config.mint)]
    pub payout: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [AGENT_SEED, agent.id.to_le_bytes().as_ref()],
        bump = agent.bump,
        has_one = operator @ Relay402Error::Unauthorized,
        has_one = payout @ Relay402Error::InvalidPayout
    )]
    pub agent: Account<'info, Agent>,

    #[account(
        mut,
        seeds = [
            RECEIPT_SEED,
            agent.key().as_ref(),
            receipt.client.as_ref(),
            receipt.nonce.to_le_bytes().as_ref()
        ],
        bump = receipt.bump,
        has_one = agent @ Relay402Error::ReceiptAgentMismatch
    )]
    pub receipt: Account<'info, Receipt>,

    #[account(address = config.mint)]
    pub mint: Account<'info, Mint>,

    #[account(mut, address = config.vault)]
    pub vault: Account<'info, TokenAccount>,

    #[account(mut, address = config.treasury @ Relay402Error::InvalidTreasury)]
    pub treasury: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn settle_payment_handler(ctx: Context<SettlePayment>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let receipt = &mut ctx.accounts.receipt;
    let agent = &mut ctx.accounts.agent;

    require!(
        receipt.status == ReceiptStatus::Pending,
        Relay402Error::PaymentNotPending
    );
    // Strictly before expiry. Refund is allowed at or after expiry, so the
    // two paths can never both succeed for the same receipt.
    require!(now < receipt.expires_at, Relay402Error::PaymentExpired);

    let amount = receipt.amount;
    let fee = compute_fee(amount, receipt.fee_bps)?;
    let net = amount.checked_sub(fee).ok_or(Relay402Error::MathOverflow)?;

    // State first, then CPIs.
    receipt.status = ReceiptStatus::Settled;
    receipt.settled_at = now;
    agent.pending_receipts = agent
        .pending_receipts
        .checked_sub(1)
        .ok_or(Relay402Error::MathOverflow)?;
    agent.settled_count = agent
        .settled_count
        .checked_add(1)
        .ok_or(Relay402Error::MathOverflow)?;

    if net > 0 {
        transfer_from_vault(
            &ctx.accounts.token_program,
            &ctx.accounts.vault,
            &ctx.accounts.mint,
            &ctx.accounts.payout,
            &ctx.accounts.config,
            net,
        )?;
    }
    if fee > 0 {
        transfer_from_vault(
            &ctx.accounts.token_program,
            &ctx.accounts.vault,
            &ctx.accounts.mint,
            &ctx.accounts.treasury,
            &ctx.accounts.config,
            fee,
        )?;
    }

    emit!(PaymentSettled {
        receipt: ctx.accounts.receipt.key(),
        agent: ctx.accounts.agent.key(),
        client: ctx.accounts.receipt.client,
        amount,
        fee,
        net,
    });
    Ok(())
}

// ------------------------------------------------------------------
// refund_payment
// ------------------------------------------------------------------

#[derive(Accounts)]
pub struct RefundPayment<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        seeds = [AGENT_SEED, agent.id.to_le_bytes().as_ref()],
        bump = agent.bump
    )]
    pub agent: Account<'info, Agent>,

    #[account(
        mut,
        seeds = [
            RECEIPT_SEED,
            agent.key().as_ref(),
            client.key().as_ref(),
            receipt.nonce.to_le_bytes().as_ref()
        ],
        bump = receipt.bump,
        has_one = agent @ Relay402Error::ReceiptAgentMismatch,
        has_one = client @ Relay402Error::ReceiptClientMismatch,
        close = client
    )]
    pub receipt: Account<'info, Receipt>,

    #[account(address = config.mint)]
    pub mint: Account<'info, Mint>,

    #[account(mut, token::mint = mint, token::authority = client)]
    pub client_token: Account<'info, TokenAccount>,

    #[account(mut, address = config.vault)]
    pub vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn refund_payment_handler(ctx: Context<RefundPayment>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let receipt = &ctx.accounts.receipt;

    require!(
        receipt.status == ReceiptStatus::Pending,
        Relay402Error::PaymentNotPending
    );
    require!(now >= receipt.expires_at, Relay402Error::PaymentNotExpired);

    let amount = receipt.amount;
    let agent = &mut ctx.accounts.agent;
    agent.pending_receipts = agent
        .pending_receipts
        .checked_sub(1)
        .ok_or(Relay402Error::MathOverflow)?;

    transfer_from_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.vault,
        &ctx.accounts.mint,
        &ctx.accounts.client_token,
        &ctx.accounts.config,
        amount,
    )?;

    emit!(PaymentRefunded {
        receipt: ctx.accounts.receipt.key(),
        agent: ctx.accounts.agent.key(),
        client: ctx.accounts.client.key(),
        amount,
    });
    // Receipt is closed by the `close = client` constraint after the handler.
    Ok(())
}

// ------------------------------------------------------------------
// submit_feedback
// ------------------------------------------------------------------

#[derive(Accounts)]
pub struct SubmitFeedback<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(
        mut,
        seeds = [AGENT_SEED, agent.id.to_le_bytes().as_ref()],
        bump = agent.bump
    )]
    pub agent: Account<'info, Agent>,

    /// Closed after feedback, so one settled payment gives exactly one score.
    #[account(
        mut,
        seeds = [
            RECEIPT_SEED,
            agent.key().as_ref(),
            client.key().as_ref(),
            receipt.nonce.to_le_bytes().as_ref()
        ],
        bump = receipt.bump,
        has_one = agent @ Relay402Error::ReceiptAgentMismatch,
        has_one = client @ Relay402Error::ReceiptClientMismatch,
        close = client
    )]
    pub receipt: Account<'info, Receipt>,
}

pub fn submit_feedback_handler(ctx: Context<SubmitFeedback>, score: u8) -> Result<()> {
    let receipt = &ctx.accounts.receipt;
    let agent = &mut ctx.accounts.agent;

    require!(
        receipt.status == ReceiptStatus::Settled,
        Relay402Error::PaymentNotSettled
    );
    require!(
        (MIN_SCORE..=MAX_SCORE).contains(&score),
        Relay402Error::InvalidScore
    );
    // Blocks the trivial case only. A second wallet can still rate, but every
    // score costs a real settled payment (price >= min_price, plus fee).
    require!(
        ctx.accounts.client.key() != agent.owner,
        Relay402Error::SelfFeedback
    );

    agent.feedback_count = agent
        .feedback_count
        .checked_add(1)
        .ok_or(Relay402Error::MathOverflow)?;
    agent.score_sum = agent
        .score_sum
        .checked_add(score as u64)
        .ok_or(Relay402Error::MathOverflow)?;

    emit!(FeedbackSubmitted {
        receipt: receipt.key(),
        agent: agent.key(),
        client: ctx.accounts.client.key(),
        score,
        feedback_count: agent.feedback_count,
        score_sum: agent.score_sum,
    });
    Ok(())
}

// ------------------------------------------------------------------
// close_receipt (settled receipt, client skips feedback)
// ------------------------------------------------------------------

#[derive(Accounts)]
pub struct CloseReceipt<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(
        mut,
        has_one = client @ Relay402Error::ReceiptClientMismatch,
        constraint = receipt.status == ReceiptStatus::Settled
            @ Relay402Error::PaymentNotSettled,
        close = client
    )]
    pub receipt: Account<'info, Receipt>,
}

pub fn close_receipt_handler(_ctx: Context<CloseReceipt>) -> Result<()> {
    Ok(())
}
