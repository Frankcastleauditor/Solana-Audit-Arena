use anchor_lang_v2::prelude::*;

#[error_code]
pub enum StakeError {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Insufficient SOL in vault")]
    InsufficientVaultBalance,
    #[msg("Insufficient deployed SOL")]
    InsufficientDeployed,
    #[msg("Slippage exceeded")]
    SlippageExceeded,
    #[msg("Zero supply")]
    ZeroSupply,
    #[msg("Pool already initialized")]
    AlreadyInitialized,
    #[msg("Invalid external program account")]
    InvalidExternalProgram,
    #[msg("User is blacklisted")]
    UserBlacklisted,
    #[msg("Fee recipients exceed the 500 bps cap")]
    FeesExceedCap,
    #[msg("Fee recipient account mismatch")]
    FeeRecipientMismatch,
}
