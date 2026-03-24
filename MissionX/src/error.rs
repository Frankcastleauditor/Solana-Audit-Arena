use anchor_lang::prelude::*;

#[error_code]
pub enum MissionxErrors {
    #[msg("Missionx program already initilaized")]
    AlreadyInitialized,
    
    #[msg("Missionx program not yet initialized")]
    NotInitialized,
    
    #[msg("Missionx program disabled")]
    NotEnabled,
    
    #[msg("This operation unsuported")]
    UnsupportedOperation,
    
    #[msg("Missionx is in unexpected state")]
    WrongMissionxSatus,
    
    #[msg("Moderator is not allowed")]
    ModeratorIsDisabled,

    #[msg("Price deriviation too big")]
    SlippageLimit,

    #[msg("Missionx is not in tradable state")]
    MissionxNotTradable,

    #[msg("Missionx is not yet reached migration or already migrated")]
    MissionxNotMigrationReady,

    #[msg("Grace period of trading for failed missionx is expired")]
    FailedMissionxTradeGracePeriodExpired,

    #[msg("Missionx is blocked")]
    MissionxBlocked,

    #[msg("Missionx is not blocked")]
    MissionxUnblocked,

    #[msg("Missionx trade state is unrecoverable")]
    MissionxTradeBannedNoRecovery,

    #[msg("Math error")]
    MathOverflow,
    
    #[msg("Missionx has failed by time out")]
    MissionxFailedByExpiration,
    
    #[msg("Missionx has not yet failed by time out")]
    MissionxNotFailedByExpiration,

    #[msg("Missionx already has joined 3 players")]
    TooManyPlayers,

    #[msg("Missionx player account is not matching any player")]
    IncorrectMissionxPlayerAccount,

    #[msg("Missionx payout too small")]
    MissionxPayoutTooSmall,

    #[msg("Missionx payout too big")]
    MissionxPayoutTooBig,

    #[msg("Missionx open timeframe is too big")]
    MissionxOpenDurationOutOfRange,

    #[msg("Missionx payout edges should follow min <= max")]
    MissionxPayoutMinLessMaxContraint,

    #[msg("Ipns reference must be equal to 65 bytes")]
    IpnsContrain,

    #[msg("Withdraw constraints not satisfied")]
    WithdrawIsNotAllowed
}