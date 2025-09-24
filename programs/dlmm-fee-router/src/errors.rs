use anchor_lang::prelude::*;

#[error_code]
pub enum FeeRouterError {
    #[msg("Invalid fee share BPS, must be <= 10000")]
    InvalidFeeShareBps,
    
    #[msg("Distribution window not reached (24h required)")]
    DistributionWindowNotReached,
    
    #[msg("Position would accrue base fees, only quote-only positions allowed")]
    BaseFeesNotAllowed,
    
    #[msg("Invalid pool configuration")]
    InvalidPoolConfiguration,
    
    #[msg("Math overflow")]
    MathOverflow,
    
    #[msg("Payout below minimum threshold")]
    PayoutBelowMinimum,
    
    #[msg("Daily cap exceeded")]
    DailyCapExceeded,
    
    #[msg("Invalid page number")]
    InvalidPageNumber,
    
    #[msg("Distribution already completed for this day")]
    DistributionAlreadyCompleted,
    
    #[msg("Invalid quote mint")]
    InvalidQuoteMint,
    
    #[msg("No fees to claim")]
    NoFeesToClaim,
    
    #[msg("Invalid investor data")]
    InvalidInvestorData,
    
    #[msg("Position not initialized")]
    PositionNotInitialized,
    
    #[msg("Vault already initialized")]
    VaultAlreadyInitialized,

    #[msg("Base fees detected, quote-only invariant violated")]
    BaseFeesDetected,

    #[msg("Missing investor associated token account")]
    MissingInvestorAta,

    #[msg("Unauthorized authority for this operation")]
    Unauthorized,

    #[msg("Distribution already executed for the given page")]
    PageAlreadyProcessed,

    #[msg("Day not started; call page 0 first to claim fees")]
    DayNotStarted,
}
