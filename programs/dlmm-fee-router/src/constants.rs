use anchor_lang::prelude::*;

/// PDA seeds
pub const VAULT_SEED: &[u8] = b"vault";
/// Matches spec: InvestorFeePositionOwnerPda with seeds [VAULT_SEED, vault, "investor_fee_pos_owner"]
pub const INVESTOR_FEE_POSITION_OWNER_SEED: &[u8] = b"investor_fee_pos_owner";
pub const DISTRIBUTION_STATE_SEED: &[u8] = b"distribution_state";
pub const INVESTOR_RECORD_SEED: &[u8] = b"investor_record";
pub const INVESTOR_PAGE_SEED: &[u8] = b"investor_page";
pub const TREASURY_QUOTE_SEED: &[u8] = b"treasury_quote";
pub const TREASURY_BASE_SEED: &[u8] = b"treasury_base";

/// Time constants
pub const SECONDS_PER_DAY: i64 = 86400;

/// Distribution constants
pub const MAX_INVESTORS_PER_PAGE: usize = 64;
pub const MAX_BPS: u16 = 10000;

/// Meteora DLMM V2 Program ID (mainnet)
pub const DLMM_PROGRAM_ID: Pubkey = solana_program::pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

/// Streamflow Program ID (mainnet)
pub const STREAMFLOW_PROGRAM_ID: Pubkey = solana_program::pubkey!("strmRqUCoQUgGUan5YhzUZa6KqdzwX5L6FpUxfmKg5m");
