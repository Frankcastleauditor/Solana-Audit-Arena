use anchor_lang::prelude::*;

#[error_code(offset = 0)]
pub enum TokenError {
    #[msg("Bonding curve is completed")]
    BondingCurveCompleted,
    #[msg("Not enough token reserves")]
    NotEnoughTokenReserves,
    #[msg("Token amount is zero")]
    TokenAmountZero,
    #[msg("Exceeded max sol amount")]
    ExceededMaxSolAmount,
    #[msg("Min sol amount not met")]
    MinSolAmountNotMet,
    #[msg("Min token amount not met")]
    MinTokenAmountNotMet,
    #[msg("Error transferring tokens")]
    TransferError,
    #[msg("Fee bps too high")]
    FeeBpsTooHigh,
    #[msg("Treasury tokens bps too high")]
    TokensBpsTooHigh,
    #[msg("Escape amount too high")]
    EscapeAmountTooHigh,
    #[msg("Escape amount is zero")]
    EscapeAmountZero,
    #[msg("Bonding curve is not completed")]
    BondingCurveNotCompleted,
    #[msg("Tokens fee cooldown")]
    TokensFeeCooldown,
}
