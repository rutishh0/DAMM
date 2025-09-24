use anchor_lang::prelude::*;

#[account]
#[derive(Default)]
pub struct DistributionState {
    /// Associated vault
    pub vault: Pubkey,
    
    /// Last distribution timestamp
    pub last_distribution_ts: i64,
    
    /// Current distribution day number
    pub current_day: u64,
    
    /// Amount distributed so far today
    pub daily_distributed: u64,
    
    /// Carry-over amount from previous day
    pub carry_over: u64,
    
    /// Current page being processed
    pub current_page: u32,
    
    /// Is the current day's distribution complete
    pub day_complete: bool,
    
    /// Total claimed fees for current day
    pub day_claimed_fees: u64,
    
    /// Total distributed to investors this day
    pub day_investor_total: u64,

    /// Pagination cursor to ensure idempotency across retries
    pub page_cursor: u64,

    /// Count of processed pages
    pub pages_processed: u32,

    /// Bitmap of processed pages (supports up to 128 pages per day)
    pub pages_done_mask: u128,
    
    /// Bump seed for PDA derivation
    pub bump: u8,
    
    /// Reserved space for future upgrades
    pub _reserved: [u8; 64],
}

impl DistributionState {
    pub const LEN: usize = 8 + // discriminator
        32 + // vault
        8 + // last_distribution_ts
        8 + // current_day
        8 + // daily_distributed
        8 + // carry_over
        4 + // current_page
        1 + // day_complete
        8 + // day_claimed_fees
        8 + // day_investor_total
        8 + // page_cursor
        4 + // pages_processed
        16 + // pages_done_mask
        1 + // bump
        64; // _reserved
    
    pub fn can_distribute(&self, current_ts: i64) -> bool {
        current_ts >= self.last_distribution_ts + crate::constants::SECONDS_PER_DAY
    }
    
    pub fn start_new_day(&mut self, current_ts: i64) {
        self.last_distribution_ts = current_ts;
        self.current_day += 1;
        self.daily_distributed = 0;
        self.current_page = 0;
        self.day_complete = false;
        self.day_claimed_fees = 0;
        self.day_investor_total = 0;
        self.page_cursor = 0;
        self.pages_processed = 0;
        self.pages_done_mask = 0;
    }

    pub fn is_page_done(&self, page: u32) -> bool {
        if page >= 128 { return false; }
        let bit = 1u128 << page;
        (self.pages_done_mask & bit) != 0
    }

    pub fn mark_page_done(&mut self, page: u32) {
        if page >= 128 { return; }
        let bit = 1u128 << page;
        self.pages_done_mask |= bit;
    }
}
