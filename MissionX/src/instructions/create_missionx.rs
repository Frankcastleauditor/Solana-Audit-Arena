use anchor_lang::{prelude::*, system_program};
use anchor_spl::{
    token_2022::{
        self,
        spl_token_2022::{self, extension::ExtensionType},
    },
    token_interface,
};
use std::mem::size_of;

use crate::{
    constants::{
        CONFIG_PDA_SEED, MINT_AMOUNT, MISSIONX_OPEN_DURATION_MAX, MISSIONX_OPEN_DURATION_MIN,
        MISSIONX_STATE, MISSIONX_TOKEN_VAULT,
    },
    error::MissionxErrors,
    events::MissionxCreated,
    instructions::ensure_enabled,
    state::{
        global_config::Configuration,
        missionx::{Missionx, MissionxStatus, MissionxTradeStatus},
    },
};

#[derive(Accounts)]
pub struct CreateAccounts<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [CONFIG_PDA_SEED],
        bump
    )]
    pub config: Box<Account<'info, Configuration>>,

    #[account(
        init,
        payer = user,
        seeds = [MISSIONX_STATE, &token_mint.key().to_bytes()],
        bump,
        space = 8 + size_of::<Missionx>(),
        rent_exempt = enforce
    )]
    pub missionx_state: Box<Account<'info, Missionx>>,

    /// CHECK: not required
    #[account(
        mut,
        address = config.fee_recipient
    )]
    pub fee_recipient: AccountInfo<'info>,

    #[account(
        mut,
        owner = system_program.key()
    )]
    pub token_mint: Signer<'info>,

    /// CHECK: not required
    #[account(
        executable,
        address = config.token_program
    )]
    pub token_program: AccountInfo<'info>,

    /// CHECK: not required
    #[account(
        mut,
        seeds = [MISSIONX_TOKEN_VAULT, &token_mint.key().to_bytes(), &missionx_state.key().to_bytes()],
        bump
    )]
    pub token_vault_pda: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn create_missionx(
    ctx: Context<CreateAccounts>,
    missionx_cid: [u8; 65],
    payout: u64,
    open_duration: u64,
) -> Result<()> {
    ensure_enabled(&ctx.accounts.config)?;
    require!(
        payout >= ctx.accounts.config.missionx_payout_min,
        MissionxErrors::MissionxPayoutTooSmall
    );
    require!(
        payout <= ctx.accounts.config.missionx_payout_max,
        MissionxErrors::MissionxPayoutTooBig
    );
    require!(
        (MISSIONX_OPEN_DURATION_MIN <= open_duration)
            && (open_duration <= MISSIONX_OPEN_DURATION_MAX),
        MissionxErrors::MissionxOpenDurationOutOfRange
    );
    let clock = Clock::get()?;

    let creation_fee = ctx.accounts.config.creation_fee;

    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.user.to_account_info(),
                to: ctx.accounts.missionx_state.to_account_info(),
            },
        ),
        payout,
    )?;

    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.user.to_account_info(),
                to: ctx.accounts.fee_recipient.to_account_info(),
            },
        ),
        creation_fee,
    )?;

    let mut token_extensions: Vec<ExtensionType> = vec![];

    let mint_space =
        ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(&token_extensions)?;
    let mint_balance = ctx.accounts.rent.minimum_balance(mint_space).max(1);

    system_program::create_account(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::CreateAccount {
                from: ctx.accounts.user.to_account_info(),
                to: ctx.accounts.token_mint.to_account_info(),
            },
        ),
        mint_balance,
        mint_space as u64,
        &ctx.accounts.config.token_program,
    )?;

    if let Some(metadata_authority) = ctx.accounts.config.metadata_authority {
        token_extensions.push(ExtensionType::MetadataPointer);

        token_interface::metadata_pointer_initialize(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token_interface::MetadataPointerInitialize {
                    token_program_id: ctx.accounts.token_program.to_account_info(),
                    mint: ctx.accounts.token_mint.to_account_info(),
                },
            ),
            Some(metadata_authority),
            None,
        )?;
    }

    token_2022::initialize_mint2(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token_2022::InitializeMint2 {
                mint: ctx.accounts.token_mint.to_account_info(),
            },
        ),
        9,
        &ctx.accounts.missionx_state.key().clone(),
        None,
    )?;

    let token_account_len = token_2022::get_account_data_size(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token_2022::GetAccountDataSize {
                mint: ctx.accounts.token_mint.to_account_info(),
            },
        ),
        &[ExtensionType::ImmutableOwner],
    )?;

    let token_account_rent = ctx
        .accounts
        .rent
        .minimum_balance(token_account_len as usize)
        .max(1);

    system_program::create_account(
        CpiContext {
            program: ctx.accounts.token_program.to_account_info(),
            accounts: system_program::CreateAccount {
                from: ctx.accounts.user.to_account_info(),
                to: ctx.accounts.token_vault_pda.to_account_info(),
            },
            remaining_accounts: vec![],
            signer_seeds: &[&[
                MISSIONX_TOKEN_VAULT,
                &ctx.accounts.token_mint.key().to_bytes(),
                &ctx.accounts.missionx_state.key().to_bytes(),
                &[ctx.bumps.token_vault_pda],
            ]],
        },
        token_account_rent,
        token_account_len as u64,
        ctx.accounts.token_program.key,
    )?;

    token_2022::initialize_account3(CpiContext {
        program: ctx.accounts.token_program.to_account_info(),
        accounts: token_2022::InitializeAccount3 {
            account: ctx.accounts.token_vault_pda.to_account_info(),
            mint: ctx.accounts.token_mint.to_account_info(),
            authority: ctx.accounts.missionx_state.to_account_info(),
        },
        remaining_accounts: vec![],
        signer_seeds: &[&[
            MISSIONX_TOKEN_VAULT,
            &ctx.accounts.token_mint.key().to_bytes(),
            &ctx.accounts.missionx_state.key().to_bytes(),
            &[ctx.bumps.token_vault_pda],
        ]],
    })?;

    token_2022::mint_to(
        CpiContext {
            program: ctx.accounts.token_program.to_account_info(),
            accounts: token_2022::MintTo {
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.token_vault_pda.to_account_info(),
                authority: ctx.accounts.missionx_state.to_account_info(),
            },
            remaining_accounts: vec![],
            signer_seeds: &[&[
                MISSIONX_STATE,
                &ctx.accounts.token_mint.key().to_bytes(),
                &[ctx.bumps.missionx_state],
            ]],
        },
        MINT_AMOUNT,
    )?;

    let missionx_state = &mut ctx.accounts.missionx_state;

    missionx_state.creator = ctx.accounts.user.key.clone();
    missionx_state.payout_amount = payout;
    missionx_state.missionx_status = MissionxStatus::Unverified;
    missionx_state.trade_status = MissionxTradeStatus::Closed;
    missionx_state.token_program = ctx.accounts.config.token_program.key();
    missionx_state.created_at = clock.unix_timestamp as u64;
    missionx_state.is_blocked = false;
    missionx_state.missionx_cid = missionx_cid.clone();
    missionx_state.token_mint = ctx.accounts.token_mint.key.clone();
    missionx_state.v0 = ctx.accounts.config.v0;
    missionx_state.v1 = ctx.accounts.config.v1;
    missionx_state.reserve0 = 0;
    missionx_state.reserve1 = MINT_AMOUNT - ctx.accounts.config.get_token_reserved();
    missionx_state.token_creator_payout = ctx.accounts.config.token_creator_payout;
    missionx_state.token_player_payout = ctx.accounts.config.token_player_payout;
    missionx_state.migration_threshold = ctx.accounts.config.migration_threshold;
    missionx_state.migration_fee = ctx.accounts.config.migration_fee;
    missionx_state.open_timestamp = 0;
    missionx_state.success_time = 0;
    missionx_state.open_duration = open_duration;

    msg!(
        "MissionxCreated; ID: {}, Po: {}, Ct: {}, TS: {}",
        *ctx.accounts.token_mint.key,
        payout,
        *ctx.accounts.user.key,
        clock.unix_timestamp
    );

    emit!(MissionxCreated {
        id: ctx.accounts.token_mint.key.clone(),
        creator: ctx.accounts.user.key.clone(),
        payout,
        timestamp: clock.unix_timestamp as u64
    });

    Ok(())
}

//#[cfg(feature = "test-instructions")]
#[derive(Accounts)]
pub struct CreateAccountsTest<'info> {
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

    #[account(
        mut,
        seeds = [MISSIONX_STATE, &token_mint.key().to_bytes()],
        bump
    )]
    pub missionx_state: Box<Account<'info, Missionx>>,

    /// CHECK: not required
    #[account()]
    pub token_mint: AccountInfo<'info>,
}

#[cfg(feature = "test-instructions")]
pub fn set_debug_duration(
    ctx: Context<CreateAccountsTest>,
    open_duration: Option<u64>,
) -> Result<()> {
    let missionx_state = &mut ctx.accounts.missionx_state;

    if let Some(open_duration) = open_duration {
        msg!("open_duration_set");
        missionx_state.open_duration = open_duration;
    }

    Ok(())
}
