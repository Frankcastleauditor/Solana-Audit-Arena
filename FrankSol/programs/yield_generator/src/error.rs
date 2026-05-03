use anchor_lang_v2::prelude::*;

#[error_code]
pub enum YieldError {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Insufficient principal tracked")]
    InsufficientPrincipal,
    #[msg("Partial withdraw is not allowed")]
    PartialWithdrawNotAllowed,
    #[msg("Clock moved backwards")]
    InvalidTime,
    #[msg("Yield vault has insufficient SOL")]
    InsufficientVaultBalance,
}
