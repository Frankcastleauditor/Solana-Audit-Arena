use anchor_lang::prelude::*;
use permit2::cpi::accounts::PermitTransfer as Permit2TransferAccounts;
use permit2::{self, PermitTransferFromArgs, SignatureTransferDetails};

mod context;
mod crypto;
mod errors;
mod events;
mod state;

pub use context::*;
pub use state::*;

use crypto::{hash_witness, WITNESS_TYPE_STRING};
use errors::X402Error;
use events::Settled;
use state::SETTLEMENT_AUTHORITY_SEED;

declare_id!("2BGWnSg718UGfCTj5hXPwpggojUkdggN7Y6BLWr2cRfo");

#[program]
pub mod x402 {
    use super::*;

    pub fn settle(
        ctx: Context<Settle>,
        permit: PermitTransferFromArgs,
        eth_address: [u8; 20],
        witness: Witness,
    ) -> Result<()> {
        require!(permit.permitted_amount > 0, X402Error::InvalidAmount);
        require_keys_neq!(witness.to, Pubkey::default(), X402Error::InvalidDestination);
        require_keys_eq!(
            ctx.accounts.recipient_token_account.key(),
            witness.to,
            X402Error::InvalidDestination
        );
        let clock = Clock::get()?;
        require!(
            (clock.unix_timestamp as u64) >= witness.valid_after,
            X402Error::PaymentTooEarly
        );

        let witness_hash = hash_witness(&witness);

        let settlement_authority_bump = ctx.bumps.settlement_authority;
        let signer_seeds: &[&[&[u8]]] =
            &[&[SETTLEMENT_AUTHORITY_SEED, &[settlement_authority_bump]]];

        let cpi_accounts = Permit2TransferAccounts {
            spender: ctx.accounts.settlement_authority.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            vault: ctx.accounts.vault.to_account_info(),
            vault_token_account: ctx.accounts.vault_token_account.to_account_info(),
            recipient_token_account: ctx.accounts.recipient_token_account.to_account_info(),
            nonce_bitmap: ctx.accounts.nonce_bitmap.to_account_info(),
            fee_payer: ctx.accounts.facilitator.to_account_info(),
            instructions_sysvar: ctx.accounts.instructions_sysvar.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.permit2_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );

        let transfer_details = SignatureTransferDetails {
            to: ctx.accounts.recipient_token_account.key(),
            requested_amount: permit.permitted_amount,
        };

        permit2::cpi::permit_witness_transfer_from(
            cpi_ctx,
            permit,
            transfer_details,
            eth_address,
            witness_hash,
            WITNESS_TYPE_STRING.to_string(),
        )?;

        emit!(Settled {
            eth_address,
            to: witness.to,
            amount: permit.permitted_amount,
            nonce: permit.nonce
        });
        Ok(())
    }
}
