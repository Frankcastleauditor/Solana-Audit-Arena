use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as ix_sysvar;
use solana_sdk_ids::secp256k1_program;
use tiny_keccak::{Hasher, Keccak};

use crate::errors::Permit2Error;
use crate::state::PermitTransferFromArgs;

pub const DOMAIN_TAG: &[u8] = b"solana-permit2:SignatureTransfer";

pub fn keccak256(chunks: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    for c in chunks {
        hasher.update(c);
    }
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

pub fn hash_permit_transfer_from(permit: &PermitTransferFromArgs, spender: &Pubkey) -> [u8; 32] {
    keccak256(&[
        DOMAIN_TAG,
        b"PermitTransferFrom",
        permit.permitted_token.as_ref(),
        &permit.permitted_amount.to_le_bytes(),
        spender.as_ref(),
        &permit.nonce.to_le_bytes(),
        &permit.deadline.to_le_bytes(),
    ])
}

pub fn hash_permit_witness_transfer_from(
    permit: &PermitTransferFromArgs,
    spender: &Pubkey,
    witness_hash: &[u8; 32],
) -> [u8; 32] {
    keccak256(&[
        DOMAIN_TAG,
        b"PermitWitnessTransferFrom",
        permit.permitted_token.as_ref(),
        &permit.permitted_amount.to_le_bytes(),
        spender.as_ref(),
        &permit.nonce.to_le_bytes(),
        &permit.deadline.to_le_bytes(),
        witness_hash,
    ])
}

pub fn verify_eth_signature(
    instructions_sysvar: &AccountInfo,
    eth_address: &[u8; 20],
    data_hash: &[u8; 32],
) -> Result<()> {
    let current_index = ix_sysvar::load_current_index_checked(instructions_sysvar)?;
    require!(current_index > 0, Permit2Error::MissingSecp256k1Instruction);

    let secp_ix_index = current_index - 1;
    let secp_ix =
        ix_sysvar::load_instruction_at_checked(secp_ix_index as usize, instructions_sysvar)?;

    require_keys_eq!(
        secp_ix.program_id,
        secp256k1_program::ID,
        Permit2Error::MissingSecp256k1Instruction
    );

    let offsets = parse_secp256k1_instruction(&secp_ix.data, secp_ix_index, data_hash)?;
    require!(
        &offsets.eth_address == eth_address,
        Permit2Error::InvalidSigner
    );


    Ok(())
}

struct Secp256k1Offsets {
    eth_address: [u8; 20],
    message: Vec<u8>,
}

fn parse_secp256k1_instruction(data: &[u8], self_index: u16, data_hash: &[u8; 32]) -> Result<Secp256k1Offsets> {
    const SIGNATURE_OFFSETS_SERIALIZED_SIZE: usize = 11;
    require!(
        !data.is_empty(),
        Permit2Error::MalformedSecp256k1Instruction
    );
    let num_signatures = data[0];
    require!(
        num_signatures == 1,
        Permit2Error::MalformedSecp256k1Instruction
    );

    require!(
        data.len() >= 1 + SIGNATURE_OFFSETS_SERIALIZED_SIZE,
        Permit2Error::MalformedSecp256k1Instruction
    );
    let o = &data[1..1 + SIGNATURE_OFFSETS_SERIALIZED_SIZE];

    let eth_address_offset = u16::from_le_bytes([o[3], o[4]]) as usize;
    let eth_address_ix_index = o[5];
    let message_offset = u16::from_le_bytes([o[6], o[7]]) as usize;
    let message_size = u16::from_le_bytes([o[8], o[9]]) as usize;
    let message_ix_index = o[10];

    require!(
        eth_address_ix_index as u16 == self_index,
        Permit2Error::MalformedSecp256k1Instruction
    );
    require!(
        message_ix_index as u16 == self_index,
        Permit2Error::MalformedSecp256k1Instruction
    );

    require!(
        data.len() >= eth_address_offset + 20,
        Permit2Error::MalformedSecp256k1Instruction
    );
    let mut eth_address = [0u8; 20];
    eth_address.copy_from_slice(&data[eth_address_offset..eth_address_offset + 20]);

    require!(
        data.len() >= message_offset + message_size,
        Permit2Error::MalformedSecp256k1Instruction
    );
    let message = data[message_offset..message_offset + message_size].to_vec();

    require!(
        data_hash.len() == message.len(),
        Permit2Error::InvalidSignature
    );

    Ok(Secp256k1Offsets {
        eth_address,
        message,
    })
}
