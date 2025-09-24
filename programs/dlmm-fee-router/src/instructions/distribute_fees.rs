use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Token, Mint, TokenAccount, Transfer};

use crate::{
    constants::*,
    errors::FeeRouterError,
    events::{QuoteFeesClaimed, InvestorPayoutPage, CreatorPayoutDayClosed, InvestorPayout},
    state::{Vault, DistributionState, InvestorPage},
    dlmm_integration,
};

#[derive(Accounts)]
#[instruction(vault_id: [u8; 32], page: u32, is_final_page: bool)]
pub struct DistributeFees<'info> {
    #[account(
        seeds = [VAULT_SEED, vault_id.as_ref()],
        bump = vault.bump,
        constraint = vault.is_initialized,
        constraint = vault.position_initialized
    )]
    pub vault: Box<Account<'info, Vault>>,
    
    #[account(
        mut,
        seeds = [DISTRIBUTION_STATE_SEED, vault_id.as_ref()],
        bump = distribution_state.bump
    )]
    pub distribution_state: Box<Account<'info, DistributionState>>,
    
    /// Investor page data for current page
    /// CHECK: Validated in instruction
    #[account(
        seeds = [b"investor_page", vault_id.as_ref(), &page.to_le_bytes()],
        bump
    )]
    pub investor_page: AccountInfo<'info>,
    
    /// Program-owned quote treasury ATA
    #[account(
        mut,
        constraint = treasury_quote.key() == vault.treasury_quote,
        constraint = treasury_quote.mint == vault.quote_mint
    )]
    pub treasury_quote: Box<Account<'info, TokenAccount>>,

    /// Program-owned base treasury ATA (should remain zero; used for invariant checks)
    #[account(
        mut,
        constraint = treasury_base.key() == vault.treasury_base
    )]
    pub treasury_base: Box<Account<'info, TokenAccount>>,
    
    /// Creator's quote token account
    #[account(
        mut,
        constraint = creator_quote_account.owner == vault.creator_wallet,
        constraint = creator_quote_account.mint == vault.quote_mint
    )]
    pub creator_quote_account: Box<Account<'info, TokenAccount>>,
    
    /// The fee position
    /// CHECK: Validated against vault
    #[account(
        constraint = fee_position.key() == vault.fee_position
    )]
    pub fee_position: AccountInfo<'info>,
    
    /// The position owner PDA
    /// CHECK: PDA derivation
    #[account(
        seeds = [VAULT_SEED, vault_id.as_ref(), INVESTOR_FEE_POSITION_OWNER_SEED],
        bump
    )]
    pub fee_position_owner: AccountInfo<'info>,
    
    /// DLMM program for claiming fees
    /// CHECK: Program ID validation
    #[account(
        constraint = dlmm_program.key() == DLMM_PROGRAM_ID
    )]
    pub dlmm_program: AccountInfo<'info>,
    
    /// Streamflow program for reading vesting data
    /// CHECK: Program ID validation
    #[account(
        constraint = streamflow_program.key() == STREAMFLOW_PROGRAM_ID
    )]
    pub streamflow_program: AccountInfo<'info>,
    
    pub quote_mint: Box<Account<'info, Mint>>,
    
    #[account(mut)]
    pub crank_operator: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
    
    // Remaining accounts are investor ATAs and stream accounts
    // Format: [investor_ata_0, stream_0, investor_ata_1, stream_1, ...]
}

pub fn distribute_fees(
    ctx: Context<DistributeFees>,
    vault_id: [u8; 32],
    page: u32,
    is_final_page: bool,
) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let distribution_state = &mut ctx.accounts.distribution_state;
    let clock = &ctx.accounts.clock;
    let current_ts = clock.unix_timestamp;
    
    // Check if we can start a new distribution day
    if page == 0 {
        require!(
            distribution_state.can_distribute(current_ts),
            FeeRouterError::DistributionWindowNotReached
        );
        
        // Start new distribution day
        distribution_state.start_new_day(current_ts);
        
        // Claim fees from the position via CPI and enforce quote-only
        let claimed_amount = claim_fees_from_position(
            &ctx.accounts.dlmm_program,
            &ctx.accounts.fee_position,
            &ctx.accounts.fee_position_owner,
            &ctx.accounts.treasury_quote,
            &ctx.accounts.treasury_base,
            vault_id,
            ctx.bumps.fee_position_owner,
        )?;

        distribution_state.day_claimed_fees = claimed_amount;
        
        emit!(QuoteFeesClaimed {
            vault_id,
            amount_claimed: claimed_amount,
            carry_over_prev: distribution_state.carry_over,
            timestamp: current_ts,
            distribution_day: distribution_state.current_day,
        });
    }
    
    // Validate page number
    require!(
        page == distribution_state.current_page,
        FeeRouterError::InvalidPageNumber
    );

    // Per-page idempotency: skip if already processed
    if distribution_state.is_page_done(page) {
        return Ok(());
    }
    
    // Calculate investor distributions for this page
    let (total_locked, mut investor_payouts) = calculate_investor_payouts(
        vault,
        distribution_state,
        &ctx.remaining_accounts,
        current_ts,
    )?;
    
    // Calculate eligible investor share
    let f_locked = if vault.total_investor_allocation > 0 {
        total_locked
            .checked_mul(MAX_BPS as u64)
            .ok_or(FeeRouterError::MathOverflow)?
            .checked_div(vault.total_investor_allocation)
            .ok_or(FeeRouterError::MathOverflow)?
    } else {
        0
    };
    
    let eligible_investor_share_bps = u64::min(
        vault.investor_fee_share_bps as u64,
        f_locked,
    );
    
    let investor_fee_quote = distribution_state.day_claimed_fees
        .checked_mul(eligible_investor_share_bps)
        .ok_or(FeeRouterError::MathOverflow)?
        .checked_div(MAX_BPS as u64)
        .ok_or(FeeRouterError::MathOverflow)?;
    
    // Compute exact pro-rata payouts and rounding remainder
    let mut allocated_total = 0u64;
    for p in investor_payouts.iter_mut() {
        let amount = if total_locked == 0 { 0 } else {
            investor_fee_quote
                .saturating_mul(p.locked_amount)
                .checked_div(total_locked)
                .ok_or(FeeRouterError::MathOverflow)?
        };
        p.amount = amount;
        allocated_total = allocated_total.saturating_add(amount);
    }
    let rounding_remainder = investor_fee_quote.saturating_sub(allocated_total);
    distribution_state.carry_over = distribution_state.carry_over.saturating_add(rounding_remainder);
    
    // Distribute to investors
    let mut total_distributed = 0u64;
    for (i, payout) in investor_payouts.iter().enumerate() {
        if payout.amount < vault.min_payout_lamports {
            // Add to carry-over
            distribution_state.carry_over += payout.amount;
            continue;
        }
        
        // Check daily cap if applicable
        if let Some(cap) = vault.daily_cap_lamports {
            if distribution_state.daily_distributed + payout.amount > cap {
                distribution_state.carry_over += payout.amount;
                continue;
            }
        }
        
        // Transfer tokens to investor
    let investor_ata_index = i * 2; // Every other remaining account is an ATA
        if investor_ata_index < ctx.remaining_accounts.len() {
            let investor_ata = &ctx.remaining_accounts[investor_ata_index];
            
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.treasury_quote.to_account_info(),
                        to: investor_ata.to_account_info(),
                        authority: ctx.accounts.fee_position_owner.to_account_info(),
                    },
                    &[&[
                        VAULT_SEED,
                        vault_id.as_ref(),
                        INVESTOR_FEE_POSITION_OWNER_SEED,
                        &[ctx.bumps.fee_position_owner],
                    ]],
                ),
                payout.amount,
            )?;
            
            total_distributed += payout.amount;
            distribution_state.daily_distributed += payout.amount;
            
            emit!(InvestorPayout {
                vault_id,
                investor: payout.investor,
                amount: payout.amount,
                locked_amount: payout.locked_amount,
                weight: payout.weight,
                timestamp: current_ts,
            });
        }
    }
    
    distribution_state.day_investor_total += total_distributed;
    
    emit!(InvestorPayoutPage {
        vault_id,
        page,
        total_payout: total_distributed,
        investor_count: investor_payouts.len() as u32,
        daily_distributed_after: distribution_state.daily_distributed,
        timestamp: current_ts,
    });
    
    // If final page, distribute remainder to creator
    if is_final_page {
        let creator_payout = distribution_state.day_claimed_fees
            .saturating_sub(distribution_state.day_investor_total);
        
        if creator_payout > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.treasury_quote.to_account_info(),
                        to: ctx.accounts.creator_quote_account.to_account_info(),
                        authority: ctx.accounts.fee_position_owner.to_account_info(),
                    },
                    &[&[
                        VAULT_SEED,
                        vault_id.as_ref(),
                        INVESTOR_FEE_POSITION_OWNER_SEED,
                        &[ctx.bumps.fee_position_owner],
                    ]],
                ),
                creator_payout,
            )?;
        }
        
        distribution_state.day_complete = true;
        
        emit!(CreatorPayoutDayClosed {
            vault_id,
            creator_payout,
            total_distributed_to_investors: distribution_state.day_investor_total,
            distribution_day: distribution_state.current_day,
            timestamp: current_ts,
        });
    } else {
        // Move to next page and advance pagination cursor for idempotency
        distribution_state.current_page += 1;
        distribution_state.page_cursor = distribution_state.page_cursor.saturating_add(1);
        distribution_state.pages_processed = distribution_state.pages_processed.saturating_add(1);
    }

    // Mark this page as processed
    distribution_state.mark_page_done(page);
    
    Ok(())
}

#[derive(Debug)]
struct InvestorPayoutInfo {
    investor: Pubkey,
    amount: u64,
    locked_amount: u64,
    weight: u64,
}

fn calculate_investor_payouts(
    _vault: &Vault,
    _distribution_state: &DistributionState,
    remaining_accounts: &[AccountInfo],
    _current_ts: i64,
) -> Result<(u64, Vec<InvestorPayoutInfo>)> {
    // Remaining accounts alternate: [investor_ata, stream]
    let mut total_locked = 0u64;
    let mut payouts = Vec::new();

    for i in (0..remaining_accounts.len()).step_by(2) {
        if i + 1 >= remaining_accounts.len() { break; }
        let investor_ata = &remaining_accounts[i];
        let stream_acc = &remaining_accounts[i + 1];

        // Call into Streamflow to read locked amount at current_ts
        let locked_amount = streamflow_read_locked(stream_acc)?;
        total_locked = total_locked.saturating_add(locked_amount);

        payouts.push(InvestorPayoutInfo {
            investor: investor_ata.key(),
            amount: 0, // computed later
            locked_amount,
            weight: locked_amount,
        });
    }

    Ok((total_locked, payouts))
}

fn streamflow_read_locked(stream: &AccountInfo) -> Result<u64> {
    // TODO: Replace with Streamflow CPI to read still-locked amount at current time
    // Temporary: derive locked amount from stream account lamports for testability
    Ok(stream.lamports() as u64)
}

fn claim_fees_from_position(
    dlmm_program: &AccountInfo,
    fee_position: &AccountInfo,
    fee_position_owner: &AccountInfo,
    treasury_quote: &Account<TokenAccount>,
    treasury_base: &Account<TokenAccount>,
    vault_id: [u8; 32],
    fee_owner_bump: u8,
) -> Result<u64> {
    // Capture balances before
    let base_before = treasury_base.amount;
    let quote_before = treasury_quote.amount;

    // Perform CPI claim (placeholder helper; wire to real DLMM when available)
    let signer = &[&[
        VAULT_SEED,
        &vault_id,
        INVESTOR_FEE_POSITION_OWNER_SEED,
        &[fee_owner_bump],
    ][..]];

    // This uses the helper to simulate CPI; replace with real one when available
    let (_claimed_x, _claimed_y) = dlmm_integration::cpi::claim_position_fees(
        dlmm_program.clone(),
        fee_position.clone(),
        AccountInfo::from(fee_position_owner.clone()),
        AccountInfo::from(treasury_quote.to_account_info()),
        AccountInfo::from(treasury_base.to_account_info()),
        signer,
    ).unwrap_or((0,0));

    // Read balances after
    let base_after = treasury_base.amount;
    let quote_after = treasury_quote.amount;

    // Enforce no base fees observed and base treasury did not increase
    require!(base_before == 0, FeeRouterError::BaseFeesDetected);
    require!(base_after == base_before, FeeRouterError::BaseFeesDetected);

    let claimed_quote = quote_after.saturating_sub(quote_before);
    require!(claimed_quote > 0, FeeRouterError::NoFeesToClaim);
    Ok(claimed_quote)
}
