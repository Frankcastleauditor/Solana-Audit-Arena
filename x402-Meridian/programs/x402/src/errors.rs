use anchor_lang::prelude::*;

#[error_code]
pub enum X402Error {
    #[msg("Settlement amount must be non-zero")]
    InvalidAmount,
    #[msg("Destination token account must not be the default pubkey")]
    InvalidDestination,
    #[msg("Payment attempted before validAfter timestamp")]
    PaymentTooEarly,
}
