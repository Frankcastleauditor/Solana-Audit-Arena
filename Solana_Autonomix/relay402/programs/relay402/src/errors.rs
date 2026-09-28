use anchor_lang::prelude::*;

#[error_code]
pub enum Relay402Error {
    #[msg("Signer is not authorized for this action")]
    Unauthorized,
    #[msg("Protocol is paused")]
    Paused,
    #[msg("Fee exceeds the maximum allowed")]
    FeeTooHigh,
    #[msg("Minimum price must be greater than zero")]
    InvalidMinPrice,
    #[msg("Price is below the protocol minimum")]
    PriceBelowMinimum,
    #[msg("Agent price is higher than the client's max amount")]
    PriceAboveMax,
    #[msg("Endpoint is empty or too long")]
    InvalidEndpoint,
    #[msg("Metadata URI is empty or too long")]
    InvalidMetadataUri,
    #[msg("Operator cannot be the default public key")]
    InvalidOperator,
    #[msg("Payout account is invalid")]
    InvalidPayout,
    #[msg("Treasury account is invalid")]
    InvalidTreasury,
    #[msg("New authority cannot be the default public key")]
    InvalidAuthority,
    #[msg("No pending transfer for this signer")]
    NoPendingTransfer,
    #[msg("Agent is not active")]
    AgentInactive,
    #[msg("Agent must be inactive with no pending payments to be closed")]
    AgentNotClosable,
    #[msg("Payment window is out of bounds")]
    InvalidPaymentWindow,
    #[msg("Payment is not pending")]
    PaymentNotPending,
    #[msg("Payment is not settled")]
    PaymentNotSettled,
    #[msg("Payment has expired")]
    PaymentExpired,
    #[msg("Payment has not expired yet")]
    PaymentNotExpired,
    #[msg("Receipt does not belong to this agent")]
    ReceiptAgentMismatch,
    #[msg("Receipt does not belong to this client")]
    ReceiptClientMismatch,
    #[msg("Score is out of range")]
    InvalidScore,
    #[msg("Agent owner cannot rate their own agent")]
    SelfFeedback,
    #[msg("Arithmetic overflow")]
    MathOverflow,
}
