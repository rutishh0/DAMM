use anchor_lang::prelude::*;

#[account]
#[derive(Default)]
pub struct InvestorRecord {
    /// Associated vault
    pub vault: Pubkey,
    
    /// Investor wallet address
    pub investor: Pubkey,
    
    /// Streamflow stream pubkey for this investor
    pub stream_pubkey: Pubkey,
    
    /// Initial allocation amount
    pub initial_allocation: u64,
    
    /// Total fees received
    pub total_fees_received: u64,
    
    /// Last distribution timestamp for this investor
    pub last_distribution_ts: i64,
    
    /// Page number this investor belongs to
    pub page: u32,
    
    /// Index within the page
    pub page_index: u32,
    
    /// Bump seed for PDA derivation
    pub bump: u8,
    
    /// Reserved space for future upgrades
    pub _reserved: [u8; 32],
}

impl InvestorRecord {
    pub const LEN: usize = 8 + // discriminator
        32 + // vault
        32 + // investor
        32 + // stream_pubkey
        8 + // initial_allocation
        8 + // total_fees_received
        8 + // last_distribution_ts
        4 + // page
        4 + // page_index
        1 + // bump
        32; // _reserved
}

/// Aggregated investor data for a page
#[account]
pub struct InvestorPage {
    /// Associated vault
    pub vault: Pubkey,
    
    /// Page number
    pub page: u32,
    
    /// Number of investors in this page
    pub investor_count: u32,
    
    /// List of investor records (pubkeys)
    pub investors: Vec<Pubkey>,
    
    /// Total locked amount for this page (cached for efficiency)
    pub total_locked: u64,
    
    /// Last update timestamp
    pub last_update_ts: i64,
    
    /// Bump seed for PDA derivation
    pub bump: u8,
}

impl InvestorPage {
    pub const BASE_LEN: usize = 8 + // discriminator
        32 + // vault
        4 + // page
        4 + // investor_count
        4 + // Vec length prefix
        8 + // total_locked
        8 + // last_update_ts
        1; // bump
    
    pub fn len(investor_count: usize) -> usize {
        Self::BASE_LEN + (investor_count * 32) // Each pubkey is 32 bytes
    }
}
