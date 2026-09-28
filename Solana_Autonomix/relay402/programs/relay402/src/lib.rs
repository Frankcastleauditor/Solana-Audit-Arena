// ============================================================
// Program:    Relay402
// Framework:  Anchor 0.31.1
// Testing:    TypeScript/Anchor (solana-test-validator)
// Risk Level: Critical (shared escrow vault, admin key, off-chain settlement)
// Summary:    Agent registry (ERC-8004 style identity + reputation) with
//             x402 pay-per-request escrow, settled by the agent's operator
//             key through an off-chain facilitator.
// ============================================================

use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use instructions::*;

declare_id!("4czqgopa31kxZtirKekBDSysjcYDUHSCC9VTVeNr4qA5");

#[program]
pub mod relay402 {
    use super::*;

    // ---------------- admin ----------------

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        fee_bps: u16,
        min_price: u64,
    ) -> Result<()> {
        instructions::admin::initialize_config_handler(ctx, fee_bps, min_price)
    }

    pub fn update_config(
        ctx: Context<UpdateConfig>,
        fee_bps: Option<u16>,
        min_price: Option<u64>,
        paused: Option<bool>,
    ) -> Result<()> {
        instructions::admin::update_config_handler(ctx, fee_bps, min_price, paused)
    }

    pub fn set_treasury(ctx: Context<SetTreasury>) -> Result<()> {
        instructions::admin::set_treasury_handler(ctx)
    }

    pub fn propose_admin(ctx: Context<ProposeAdmin>, new_admin: Pubkey) -> Result<()> {
        instructions::admin::propose_admin_handler(ctx, new_admin)
    }

    pub fn accept_admin(ctx: Context<AcceptAdmin>) -> Result<()> {
        instructions::admin::accept_admin_handler(ctx)
    }

    // ---------------- agents ----------------

    pub fn register_agent(ctx: Context<RegisterAgent>, args: RegisterAgentArgs) -> Result<()> {
        instructions::agent::register_agent_handler(ctx, args)
    }

    pub fn update_agent(ctx: Context<UpdateAgent>, args: UpdateAgentArgs) -> Result<()> {
        instructions::agent::update_agent_handler(ctx, args)
    }

    pub fn set_payout(ctx: Context<SetPayout>) -> Result<()> {
        instructions::agent::set_payout_handler(ctx)
    }

    pub fn set_agent_active(ctx: Context<SetAgentActive>, active: bool) -> Result<()> {
        instructions::agent::set_agent_active_handler(ctx, active)
    }

    pub fn transfer_agent(ctx: Context<TransferAgent>, new_owner: Pubkey) -> Result<()> {
        instructions::agent::transfer_agent_handler(ctx, new_owner)
    }

    pub fn accept_agent(ctx: Context<AcceptAgent>) -> Result<()> {
        instructions::agent::accept_agent_handler(ctx)
    }

    pub fn close_agent(ctx: Context<CloseAgent>) -> Result<()> {
        instructions::agent::close_agent_handler(ctx)
    }

    // ---------------- payments ----------------

    pub fn create_payment(
        ctx: Context<CreatePayment>,
        nonce: u64,
        window_secs: i64,
        max_amount: u64,
    ) -> Result<()> {
        instructions::payment::create_payment_handler(ctx, nonce, window_secs, max_amount)
    }

    pub fn settle_payment(ctx: Context<SettlePayment>) -> Result<()> {
        instructions::payment::settle_payment_handler(ctx)
    }

    pub fn refund_payment(ctx: Context<RefundPayment>) -> Result<()> {
        instructions::payment::refund_payment_handler(ctx)
    }

    pub fn submit_feedback(ctx: Context<SubmitFeedback>, score: u8) -> Result<()> {
        instructions::payment::submit_feedback_handler(ctx, score)
    }

    pub fn close_receipt(ctx: Context<CloseReceipt>) -> Result<()> {
        instructions::payment::close_receipt_handler(ctx)
    }
}
