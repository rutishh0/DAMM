use anchor_lang::prelude::*;

#[event]
pub struct VaultInitialized {
    pub vault_id: [u8; 32],
    pub creator: Pubkey,
    pub investor_fee_share_bps: u16,
    pub min_payout_lamports: u64,
    pub daily_cap_lamports: Option<u64>,
    pub timestamp: i64,
}

#[event]
pub struct HonoraryPositionInitialized {
    pub vault_id: [u8; 32],
    pub position_pubkey: Pubkey,
    pub pool_pubkey: Pubkey,
    pub quote_mint: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct QuoteFeesClaimed {
    pub vault_id: [u8; 32],
    pub amount_claimed: u64,
    pub carry_over_prev: u64,
    pub timestamp: i64,
    pub distribution_day: u64,
}

#[event]
pub struct InvestorPayoutPage {
    pub vault_id: [u8; 32],
    pub page: u32,
    pub total_payout: u64,
    pub investor_count: u32,
    pub daily_distributed_after: u64,
    pub timestamp: i64,
}

#[event]
pub struct CreatorPayoutDayClosed {
    pub vault_id: [u8; 32],
    pub creator_payout: u64,
    pub total_distributed_to_investors: u64,
    pub distribution_day: u64,
    pub timestamp: i64,
}

#[event]
pub struct InvestorPayout {
    pub vault_id: [u8; 32],
    pub investor: Pubkey,
    pub amount: u64,
    pub locked_amount: u64,
    pub weight: u64,
    pub timestamp: i64,
}
