use anchor_lang::prelude::*;
use std::mem::size_of;

use crate::{constants::{CONFIG_PDA_SEED, MISSIONX_MODERATOR}, events::ModeratorUpdate, state::{global_config::Configuration, moderator::ModeratorState}};

use super::ensure_enabled;

#[derive(Accounts)]
pub struct UpdateModearatorAccounts<'info> {
    #[account(
        mut,
        address = config.owner
    )]
    pub owner: Signer<'info>,

    /// CHECK: not required
    #[account()]
    pub moderator: AccountInfo<'info>,

    #[account(
        seeds = [CONFIG_PDA_SEED],
        bump
    )]
    pub config: Box<Account<'info, Configuration>>,
    
    #[account(
        init_if_needed,
        payer = owner,
        seeds = [MISSIONX_MODERATOR, &moderator.key.to_bytes()],
        bump,
        space = 8 + size_of::<ModeratorState>(),
        rent_exempt = enforce
    )]
    pub moderator_config: Box<Account<'info, ModeratorState>>,

    pub system_program: Program<'info, System>,
}

pub fn update_moderator(
    ctx: Context<UpdateModearatorAccounts>,
    enable_moderator: bool
) -> Result<()> {
    ensure_enabled(&ctx.accounts.config)?;

    let moderator_data = &mut ctx.accounts.moderator_config;

    moderator_data.is_enabled = enable_moderator;

    msg!("ModeratorUpdate; Moderator: {}, enabled: {}",
        ctx.accounts.moderator.key(),
        enable_moderator,
    );
    emit!(ModeratorUpdate{
        moderator: ctx.accounts.moderator.key(),
        enabled: enable_moderator
    });

    Ok(())
}