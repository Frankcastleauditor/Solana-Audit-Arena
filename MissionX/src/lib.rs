pub mod instructions;
pub mod state;
pub mod error;
pub mod constants;
pub mod events;
pub mod utils;

use anchor_lang::prelude::*;
use instructions::*;

declare_id!("79Ltf6NgMwa7pdyBqTuyHmmoeHJ5bRYZsBRuWfa4voHG");


#[program]
pub mod missionx_tokens {
    #[cfg(not(feature = "test-instructions"))]
    use error::MissionxErrors;

    use super::*;

    pub fn init(
        ctx: Context<InitAccounts>,
        is_enabled: bool,
        missionx_payout_min: u64,
        missionx_payout_max: u64,
        creation_fee: u64,
        fee_recipient: Pubkey,
        token_program: Pubkey,
        v0: u64,
        v1: u64,
        metadata_authority: Option<Pubkey>,
        executor: Pubkey,
        ipns_root: [u8; 65],
    ) -> Result<()> {
        init::init(ctx, is_enabled, missionx_payout_min, missionx_payout_max, creation_fee, fee_recipient, token_program, v0, v1, metadata_authority, executor, ipns_root)
    }

    pub fn set_options(
        ctx: Context<SetConfigAccounts>,
        owner: Option<Pubkey>,
        is_enabled: Option<bool>,
        missionx_payout_min: Option<u64>,
        missionx_payout_max: Option<u64>,
        creation_fee: Option<u64>,
        fee_recipient: Option<Pubkey>,
        token_program: Option<Pubkey>,
        v0: Option<u64>,
        v1: Option<u64>,
        token_player_payout: Option<u64>,
        token_creator_payout: Option<u64>,
        metadata_authority: Option<Pubkey>,
        migration_threshold: Option<u64>,
        migration_fee: Option<u64>,
        executor: Option<Pubkey>,
        ipns_root: Option<[u8; 65]>,
        fail_grace_period: Option<u64>,
        fail_fee: Option<u64>,
        trade_fee_bps: Option<u64>,
    ) -> Result<()> {
        set_global_config::set_options(ctx, owner, is_enabled, missionx_payout_min, missionx_payout_max, creation_fee, fee_recipient, token_program, v0, v1, token_player_payout, token_creator_payout, metadata_authority, migration_threshold, migration_fee, executor, ipns_root, fail_grace_period, fail_fee, trade_fee_bps)
    }

    pub fn update_moderator(
        ctx: Context<UpdateModearatorAccounts>,
        enable_moderator: bool
    ) -> Result<()> {
        update_moderator::update_moderator(ctx, enable_moderator)
    }

    pub fn create_missionx(
        ctx: Context<CreateAccounts>,
        missionx_cid: [u8; 65],
        payout: u64,
        open_duration: u64,
    ) -> Result<()> {
        create_missionx::create_missionx(ctx, missionx_cid, payout, open_duration)
    }

    pub fn moderate_initial(
        ctx: Context<ModearateAccounts>,
        block: bool,
    ) -> Result<()> {
        moderate::moderate_initial(ctx, block)
    }

    pub fn ban_active(
        ctx: Context::<ModearateAccounts>,
        ban_sell: bool,
    ) -> Result<()> {
        moderate::ban_active(ctx, ban_sell)
    }

    pub fn unban_active(
        ctx: Context::<ModearateAccounts>,
    ) -> Result<()> {
        moderate::unban_active(ctx)
    }
    
    pub fn switch_ban_to_failed(
        ctx: Context<ModearateAccounts>,
        immediate: bool
    ) -> Result<()> {
        moderate::switch_ban_to_failed(ctx, immediate)
    }

    pub fn accept_missionx(
        ctx: Context::<AcceptAccounts>,
    ) -> Result<()> {
        accept::accept_missionx(ctx)
    }

    pub fn accept_missionx_multi(
        ctx: Context::<AcceptAccounts>,
    ) -> Result<()> {
        accept::accept_missionx_multi(ctx)
    }

    pub fn complete_missionx(
        ctx: Context::<ConfirmAccounts>,
        is_successful: bool,
    ) -> Result<()> {
        complete::complete_missionx(ctx, is_successful)
    }

    pub fn buy(
        ctx: Context::<BuyAccounts>,
        buy_amount: u64,
        pay_cap: u64,
    ) -> Result<()> {
        buy::buy(ctx, buy_amount, pay_cap)
    }

    pub fn sell(
        ctx: Context::<SellAccounts>,
        sell_amount: u64,
        min_out: u64,
    ) -> Result<()> {
        sell::sell(ctx, sell_amount, min_out)
    }
    
    pub fn migrate(
        ctx: Context::<MigrateAccounts>,
    ) -> Result<()> {
        migrate::migrate(ctx)
    }

    pub fn fail_missionx(
        ctx: Context<FailAccounts>,
    ) -> Result<()> {
        fail_missionx::fail_missionx(ctx)
    }

    pub fn fail_missionx_by_time(
        ctx: Context<FailAccountsByTime>,
    ) -> Result<()> {
        fail_missionx::fail_missionx_by_time(ctx)
    }
    
    pub fn withdraw_from_missionx(
        ctx: Context<WithdrawAccounts>,
    ) -> Result<()> {
        withdraw::withdraw_from_missionx(ctx)
    }





    
    pub fn set_debug_duration(
        ctx: Context<CreateAccountsTest>,
        open_duration: Option<u64>,
    ) -> Result<()> {
        #[cfg(feature = "test-instructions")]
        return create_missionx::set_debug_duration(ctx, open_duration);

        #[cfg(not(feature = "test-instructions"))]
        {
            require!(false, MissionxErrors::UnsupportedOperation);
            Ok(())
        }
    }
}

#[derive(Accounts)]
pub struct Initialize {}
