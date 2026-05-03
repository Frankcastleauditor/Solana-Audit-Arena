pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang_v2::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("tsXqrLjaTSNeyCEMTBAHPZkyDvxRHBuv1t29f3rkZpz");

#[event]
pub struct StakeEvent {
    pub user: Address,
    pub sol_deposited: u64,
    pub franksol_minted: u64,
    pub pool_total_sol: u64,
    pub franksol_supply: u64,
}

#[program]
pub mod stake_v2 {
    use super::*;

    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        initialize::handler(ctx)
    }

    pub fn stake(
        ctx: &mut Context<Stake>,
        amount_sol: u64,
        min_franksol_out: u64,
    ) -> Result<()> {
        stake::handler(ctx, amount_sol, min_franksol_out)
    }

    pub fn unstake(
        ctx: &mut Context<Unstake>,
        franksol_in: u64,
        min_sol_out: u64,
    ) -> Result<()> {
        unstake::handler(ctx, franksol_in, min_sol_out)
    }

    pub fn deploy_to_yield(ctx: &mut Context<DeployToYield>, amount: u64) -> Result<()> {
        deploy_to_yield::handler(ctx, amount)
    }

    pub fn withdraw_from_yield(
        ctx: &mut Context<WithdrawFromYield>,
        principal_returned: u64,
        yield_amount: u64,
    ) -> Result<()> {
        withdraw_from_yield::handler(ctx, principal_returned, yield_amount)
    }

    pub fn set_admin(ctx: &mut Context<SetAdmin>, new_admin: Address) -> Result<()> {
        admin::set_admin(ctx, new_admin)
    }

    pub fn set_fund_manager(ctx: &mut Context<SetFundManager>, new_fm: Address) -> Result<()> {
        admin::set_fund_manager(ctx, new_fm)
    }

    pub fn set_fee_recipients(
        ctx: &mut Context<SetFeeRecipients>,
        slots: [FeeRecipientArg; 3],
    ) -> Result<()> {
        admin::set_fee_recipients(ctx, slots)
    }

    pub fn set_user_blacklist(
        ctx: &mut Context<SetUserBlacklist>,
        is_blacklisted: bool,
    ) -> Result<()> {
        admin::set_user_blacklist(ctx, is_blacklisted)
    }
}
