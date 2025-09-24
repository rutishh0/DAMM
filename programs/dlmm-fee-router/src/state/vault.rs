use anchor_lang::prelude::*;

#[account]
#[derive(Default)]
pub struct Vault {
    /// Unique vault identifier
    pub vault_id: [u8; 32],
    
    /// The creator wallet that receives remainder fees
    pub creator_wallet: Pubkey,
    
    /// The DLMM pool pubkey
    pub pool: Pubkey,
    
    /// The quote mint (usually USDC)
    pub quote_mint: Pubkey,
    
    /// The honorary position pubkey
    pub fee_position: Pubkey,
    
    /// Investor fee share in basis points (max 10000)
    pub investor_fee_share_bps: u16,
    
    /// Minimum payout amount (dust threshold)
    pub min_payout_lamports: u64,
    
    /// Optional daily distribution cap
    pub daily_cap_lamports: Option<u64>,
    
    /// Total initial allocation for investors (Y0)
    pub total_investor_allocation: u64,

    /// Treasury ATAs for quote and base (base used only for invariant checks)
    pub treasury_quote: Pubkey,
    pub treasury_base: Pubkey,
    
    /// Is the vault initialized
    pub is_initialized: bool,
    
    /// Is the fee position created
    pub position_initialized: bool,
    
    /// Bump seed for PDA derivation
    pub bump: u8,
    
    /// Reserved space for future upgrades
    pub _reserved: [u8; 32],
}

impl Vault {
    pub const LEN: usize = 8 + // discriminator
        32 + // vault_id
        32 + // creator_wallet
        32 + // pool
        32 + // quote_mint
        32 + // fee_position
        2 + // investor_fee_share_bps
        8 + // min_payout_lamports
        1 + 8 + // Option<daily_cap_lamports>
        8 + // total_investor_allocation
        32 + // treasury_quote
        32 + // treasury_base
        1 + // is_initialized
        1 + // position_initialized
        1 + // bump
        32; // _reserved
}
