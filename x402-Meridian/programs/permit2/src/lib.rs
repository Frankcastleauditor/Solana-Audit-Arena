use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, TransferChecked};

mod context;
mod crypto;
mod errors;
mod events;
mod instructions;
mod state;

pub use context::*;
pub use state::*;

use crypto::{
    hash_permit_transfer_from, hash_permit_witness_transfer_from, keccak256, verify_eth_signature,
    DOMAIN_TAG,
};
use errors::Permit2Error;
use events::{Deposited, NonceBitmapClosed, UnorderedNonceInvalidation};
use instructions::permit_transfer_from_common;

declare_id!("dhmiZvXpoLv2aqEaR19FiP2od3avCA7NTvQfUVNnN6U");

#[program]
pub mod permit2 {
    use super::*;

    pub fn init_vault(ctx: Context<InitVault>, eth_address: [u8; 20]) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        vault.eth_address = eth_address;
        vault.mint = ctx.accounts.mint.key();
        vault.bump = ctx.bumps.vault;
        vault.token_account = ctx.accounts.vault_token_account.key();
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, _eth_address: [u8; 20], amount: u64) -> Result<()> {
        require!(amount > 0, Permit2Error::InvalidAmount);

        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.depositor_token_account.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.vault_token_account.to_account_info(),
                    authority: ctx.accounts.depositor.to_account_info(),
                },
            ),
            amount,
            ctx.accounts.mint.decimals,
        )?;

        emit!(Deposited {
            eth_address: _eth_address,
            mint: ctx.accounts.mint.key(),
            depositor: ctx.accounts.depositor.key(),
            amount,
        });
        Ok(())
    }

    pub fn permit_transfer_from(
        ctx: Context<PermitTransfer>,
        permit: PermitTransferFromArgs,
        transfer_details: SignatureTransferDetails,
        eth_address: [u8; 20],
    ) -> Result<()> {
        let data_hash = hash_permit_transfer_from(&permit, &ctx.accounts.spender.key());
        permit_transfer_from_common(ctx, permit, transfer_details, eth_address, data_hash)
    }

    pub fn permit_witness_transfer_from(
        ctx: Context<PermitTransfer>,
        permit: PermitTransferFromArgs,
        transfer_details: SignatureTransferDetails,
        eth_address: [u8; 20],
        witness_hash: [u8; 32],
        _witness_type_string: String,
    ) -> Result<()> {
        let data_hash =
            hash_permit_witness_transfer_from(&permit, &ctx.accounts.spender.key(), &witness_hash);
        permit_transfer_from_common(ctx, permit, transfer_details, eth_address, data_hash)
    }

    pub fn invalidate_unordered_nonces(
        ctx: Context<InvalidateNonces>,
        eth_address: [u8; 20],
        word_pos: u64,
        mask_lo: u128,
        mask_hi: u128,
    ) -> Result<()> {
        let message_hash = keccak256(&[
            DOMAIN_TAG,
            b"InvalidateUnorderedNonces",
            eth_address.as_ref(),
            &word_pos.to_le_bytes(),
            &mask_lo.to_le_bytes(),
            &mask_hi.to_le_bytes(),
        ]);
        verify_eth_signature(
            &ctx.accounts.instructions_sysvar,
            &eth_address,
            &message_hash,
        )?;

        let bitmap = &mut ctx.accounts.nonce_bitmap;
        if bitmap.word_lo == 0 && bitmap.word_hi == 0 && bitmap.eth_address == [0u8; 20] {
            bitmap.rent_payer = ctx.accounts.payer.key();
        }
        bitmap.eth_address = eth_address;
        bitmap.word_pos = word_pos;
        bitmap.word_lo |= mask_lo;
        bitmap.word_hi |= mask_hi;
        bitmap.bump = ctx.bumps.nonce_bitmap;

        emit!(UnorderedNonceInvalidation {
            eth_address,
            word_pos,
            mask_lo,
            mask_hi,
        });
        Ok(())
    }

    pub fn close_nonce_bitmap(
        ctx: Context<CloseNonceBitmap>,
        eth_address: [u8; 20],
        word_pos: u64,
    ) -> Result<()> {
        require!(
            ctx.accounts.nonce_bitmap.eth_address == eth_address,
            Permit2Error::EthAddressMismatch
        );
        require!(
            ctx.accounts.nonce_bitmap.word_pos == word_pos,
            Permit2Error::EthAddressMismatch
        );
        require!(
            ctx.accounts.nonce_bitmap.word_lo == u128::MAX
                && ctx.accounts.nonce_bitmap.word_hi == u128::MAX,
            Permit2Error::NonceBitmapNotFullyConsumed
        );

        emit!(NonceBitmapClosed {
            eth_address,
            word_pos,
            rent_payer: ctx.accounts.rent_payer.key(),
        });
        Ok(())
    }
}
