use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod dlmm_integration;

use instructions::*;

declare_id!("FeeRouter11111111111111111111111111111111111");

#[program]
pub mod dlmm_fee_router {
    use super::*;

    /// Initialize the fee router vault configuration
    pub fn initialize_vault(
        ctx: Context<InitializeVault>,
        vault_id: [u8; 32],
        creator_wallet: Pubkey,
        investor_fee_share_bps: u16,
        min_payout_lamports: u64,
        daily_cap_lamports: Option<u64>,
    ) -> Result<()> {
        instructions::initialize_vault(
            ctx,
            vault_id,
            creator_wallet,
            investor_fee_share_bps,
            min_payout_lamports,
            daily_cap_lamports,
        )
    }

    /// Initialize the honorary fee position for quote-only fees
    pub fn initialize_fee_position(
        ctx: Context<InitializeFeePosition>,
        vault_id: [u8; 32],
    ) -> Result<()> {
        instructions::initialize_fee_position(ctx, vault_id)
    }

    /// Claim fees and distribute to investors (paginated, once per 24h)
    pub fn distribute_fees(
        ctx: Context<DistributeFees>,
        vault_id: [u8; 32],
        page: u32,
        is_final_page: bool,
    ) -> Result<()> {
        instructions::distribute_fees(ctx, vault_id, page, is_final_page)
    }

    /// Update investor allocation data (called when needed)
    pub fn update_investor_data(
        ctx: Context<UpdateInvestorData>,
        vault_id: [u8; 32],
        total_allocation: u64,
    ) -> Result<()> {
        instructions::update_investor_data(ctx, vault_id, total_allocation)
    }
}
