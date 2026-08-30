use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, TransferChecked};

use crate::context::PermitTransfer;
use crate::crypto::verify_eth_signature;
use crate::errors::Permit2Error;
use crate::events::PermitTransferExecuted;
use crate::state::{NonceBitmap, PermitTransferFromArgs, SignatureTransferDetails, VAULT_SEED};

pub fn permit_transfer_from_common(
    ctx: Context<PermitTransfer>,
    permit: PermitTransferFromArgs,
    transfer_details: SignatureTransferDetails,
    eth_address: [u8; 20],
    data_hash: [u8; 32],
) -> Result<()> {
    require!(
        transfer_details.requested_amount <= permit.permitted_amount,
        Permit2Error::InvalidAmount
    );
    require_keys_eq!(
        ctx.accounts.mint.key(),
        permit.permitted_token,
        Permit2Error::TokenMismatch
    );
    require!(
        ctx.accounts.vault.eth_address == eth_address,
        Permit2Error::EthAddressMismatch
    );
    require_keys_eq!(
        ctx.accounts.vault.mint,
        permit.permitted_token,
        Permit2Error::TokenMismatch
    );
    require_keys_eq!(
        ctx.accounts.recipient_token_account.mint,
        permit.permitted_token,
        Permit2Error::TokenMismatch
    );
    require_keys_eq!(
        ctx.accounts.recipient_token_account.key(),
        transfer_details.to,
        Permit2Error::RecipientMismatch
    );

    let nonce_bitmap_bump = ctx.bumps.nonce_bitmap;
    let fee_payer_key = ctx.accounts.fee_payer.key();
    use_unordered_nonce(
        &mut ctx.accounts.nonce_bitmap,
        &eth_address,
        permit.nonce,
        nonce_bitmap_bump,
        fee_payer_key,
    )?;

    verify_eth_signature(&ctx.accounts.instructions_sysvar, &eth_address, &data_hash)?;

    let mint_key = ctx.accounts.mint.key();
    let seeds: &[&[u8]] = &[
        VAULT_SEED,
        eth_address.as_ref(),
        mint_key.as_ref(),
        &[ctx.accounts.vault.bump],
    ];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    if transfer_details.requested_amount > 0 {
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.recipient_token_account.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                },
                signer_seeds,
            ),
            transfer_details.requested_amount,
            ctx.accounts.mint.decimals,
        )?;
    }

    emit!(PermitTransferExecuted {
        eth_address,
        mint: mint_key,
        to: transfer_details.to,
        amount: transfer_details.requested_amount,
        nonce: permit.nonce,
    });

    Ok(())
}

pub fn use_unordered_nonce(
    bitmap: &mut Account<NonceBitmap>,
    eth_address: &[u8; 20],
    nonce: u64,
    bump: u8,
    fee_payer: Pubkey,
) -> Result<()> {
    let word_pos = nonce / 256;
    if bitmap.word_lo == 0 && bitmap.word_hi == 0 && bitmap.eth_address == [0u8; 20] {
        bitmap.eth_address = *eth_address;
        bitmap.word_pos = word_pos;
        bitmap.bump = bump;
        bitmap.rent_payer = fee_payer;
    }
    require!(
        &bitmap.eth_address == eth_address,
        Permit2Error::EthAddressMismatch
    );
    require!(
        bitmap.word_pos == word_pos,
        Permit2Error::EthAddressMismatch
    );

    let bit_pos = (nonce % 256) as u32;
    let bit = 1u128 << (bit_pos % 128);
    let half = (bit_pos / 128) as usize;

    let word_half = if half == 0 {
        &mut bitmap.word_lo
    } else {
        &mut bitmap.word_hi
    };
    let already_set = *word_half & bit != 0;
    require!(!already_set, Permit2Error::InvalidNonce);
    *word_half |= bit;
    Ok(())
}
