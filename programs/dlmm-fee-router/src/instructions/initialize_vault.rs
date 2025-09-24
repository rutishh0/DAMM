use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Token, Mint, TokenAccount};

use crate::{
    constants::*,
    errors::FeeRouterError,
    events::VaultInitialized,
    state::{Vault, DistributionState},
};

#[derive(Accounts)]
#[instruction(vault_id: [u8; 32])]
pub struct InitializeVault<'info> {
    #[account(
        init,
        payer = authority,
        space = Vault::LEN,
        seeds = [VAULT_SEED, vault_id.as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,
    
    #[account(
        init,
        payer = authority,
        space = DistributionState::LEN,
        seeds = [DISTRIBUTION_STATE_SEED, vault_id.as_ref()],
        bump
    )]
    pub distribution_state: Account<'info, DistributionState>,
    
    /// Quote mint (usually USDC)
    pub quote_mint: Account<'info, Mint>,

    /// Base mint for DLMM pair (for invariant checks only)
    pub base_mint: Account<'info, Mint>,

    /// Program-owned quote treasury ATA
    #[account(
        init,
        payer = authority,
        associated_token::mint = quote_mint,
        associated_token::authority = fee_position_owner_pda,
    )]
    pub treasury_quote: Account<'info, TokenAccount>,

    /// Program-owned base treasury ATA (should remain zero; used to detect base fees)
    #[account(
        init,
        payer = authority,
        associated_token::mint = base_mint,
        associated_token::authority = fee_position_owner_pda,
    )]
    pub treasury_base: Account<'info, TokenAccount>,

    /// PDA that will own the honorary position and treasuries
    /// CHECK: derived and used as authority only
    #[account(
        seeds = [VAULT_SEED, vault_id.as_ref(), INVESTOR_FEE_POSITION_OWNER_SEED],
        bump
    )]
    pub fee_position_owner_pda: AccountInfo<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn initialize_vault(
    ctx: Context<InitializeVault>,
    vault_id: [u8; 32],
    creator_wallet: Pubkey,
    investor_fee_share_bps: u16,
    min_payout_lamports: u64,
    daily_cap_lamports: Option<u64>,
) -> Result<()> {
    let vault = &mut ctx.accounts.vault;
    let distribution_state = &mut ctx.accounts.distribution_state;
    
    // Validate parameters
    require!(
        investor_fee_share_bps <= MAX_BPS,
        FeeRouterError::InvalidFeeShareBps
    );
    
    require!(
        !vault.is_initialized,
        FeeRouterError::VaultAlreadyInitialized
    );
    
    // Initialize vault
    vault.vault_id = vault_id;
    vault.creator_wallet = creator_wallet;
    vault.quote_mint = ctx.accounts.quote_mint.key();
    vault.investor_fee_share_bps = investor_fee_share_bps;
    vault.min_payout_lamports = min_payout_lamports;
    vault.daily_cap_lamports = daily_cap_lamports;
    vault.treasury_quote = ctx.accounts.treasury_quote.key();
    vault.treasury_base = ctx.accounts.treasury_base.key();
    vault.is_initialized = true;
    vault.position_initialized = false;
    vault.bump = ctx.bumps.vault;
    
    // Initialize distribution state
    distribution_state.vault = vault.key();
    distribution_state.last_distribution_ts = 0;
    distribution_state.current_day = 0;
    distribution_state.bump = ctx.bumps.distribution_state;
    
    emit!(VaultInitialized {
        vault_id,
        creator: creator_wallet,
        investor_fee_share_bps,
        min_payout_lamports,
        daily_cap_lamports,
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    Ok(())
}
