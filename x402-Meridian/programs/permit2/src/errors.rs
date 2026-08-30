use anchor_lang::prelude::*;

#[error_code]
pub enum Permit2Error {
    #[msg("Requested amount exceeds permitted amount")]
    InvalidAmount,
    #[msg("Signature deadline has passed")]
    SignatureExpired,
    #[msg("Nonce has already been used")]
    InvalidNonce,
    #[msg("Recovered signer does not match expected eth_address")]
    InvalidSigner,
    #[msg("Signature does not match the expected message hash")]
    InvalidSignature,
    #[msg("Vault eth_address does not match the provided eth_address")]
    EthAddressMismatch,
    #[msg("Token mint does not match permit.permitted_token")]
    TokenMismatch,
    #[msg("Recipient token account does not match transfer_details.to")]
    RecipientMismatch,
    #[msg("Preceding instruction must be a valid Secp256k1Program signature verification")]
    MissingSecp256k1Instruction,
    #[msg("Secp256k1Program instruction data is malformed")]
    MalformedSecp256k1Instruction,
    #[msg("Nonce bitmap still has unused nonces and cannot be closed")]
    NonceBitmapNotFullyConsumed,
}
