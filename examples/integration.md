# DLMM Fee Router - Integration Examples

## Complete Integration with Meteora DLMM V2

This document provides detailed integration examples for connecting the Fee Router with Meteora's DLMM V2 protocol.

## 1. Setting Up the Honorary Position

The honorary position must be configured to only accrue quote fees. Here's how to determine the correct tick range:

```typescript
import { DLMM } from '@meteora-ag/dlmm';
import { BN } from '@coral-xyz/anchor';

async function createQuoteOnlyPosition(
  pool: PublicKey,
  quoteMint: PublicKey,
  baseMint: PublicKey
) {
  // Fetch pool state
  const poolState = await dlmm.getPool(pool);
  
  // Determine if quote is token X or Y
  const isQuoteTokenX = poolState.tokenX.equals(quoteMint);
  
  // Get current price and tick
  const currentTick = poolState.activeId;
  const tickSpacing = poolState.binStep;
  
  // For quote-only fees, we need a position that:
  // 1. Is out of range (no liquidity provided)
  // 2. Is positioned to only collect quote fees
  
  let tickLower: number;
  let tickUpper: number;
  
  if (isQuoteTokenX) {
    // Quote is token X: position below current price
    // This collects fees when price moves down (quote appreciates)
    tickUpper = currentTick - tickSpacing;
    tickLower = tickUpper - (tickSpacing * 100); // Wide range
  } else {
    // Quote is token Y: position above current price
    // This collects fees when price moves up (quote appreciates)
    tickLower = currentTick + tickSpacing;
    tickUpper = tickLower + (tickSpacing * 100); // Wide range
  }
  
  return { tickLower, tickUpper };
}
```

## 2. CPI to Create DLMM Position

Here's the actual CPI implementation for creating the position:

```rust
use anchor_lang::prelude::*;
use meteora_dlmm::cpi::{accounts::InitializePosition, initialize_position};
use meteora_dlmm::state::lb_pair::LbPair;

pub fn create_honorary_position(
    ctx: Context<CreatePosition>,
    vault_id: [u8; 32],
    tick_lower: i32,
    tick_upper: i32,
) -> Result<()> {
    let seeds = &[
        VAULT_SEED,
        vault_id.as_ref(),
        FEE_POSITION_OWNER_SEED,
        &[ctx.bumps.fee_position_owner],
    ];
    let signer = &[&seeds[..]];
    
    // Create position through DLMM
    let cpi_accounts = InitializePosition {
        payer: ctx.accounts.authority.to_account_info(),
        position: ctx.accounts.position.to_account_info(),
        lb_pair: ctx.accounts.lb_pair.to_account_info(),
        owner: ctx.accounts.fee_position_owner.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
        rent: ctx.accounts.rent.to_account_info(),
        event_authority: ctx.accounts.event_authority.to_account_info(),
        program: ctx.accounts.dlmm_program.to_account_info(),
    };
    
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.dlmm_program.to_account_info(),
        cpi_accounts,
        signer,
    );
    
    // Initialize with 0 liquidity (honorary position)
    initialize_position(
        cpi_ctx,
        tick_lower,
        tick_upper,
        0, // No liquidity - fee accrual only
    )?;
    
    Ok(())
}
```

## 3. Claiming Fees from DLMM

Implementation for claiming accumulated fees:

```rust
use meteora_dlmm::cpi::{accounts::ClaimFee, claim_fee};

pub fn claim_position_fees(
    ctx: Context<ClaimFees>,
    vault_id: [u8; 32],
) -> Result<u64> {
    let seeds = &[
        VAULT_SEED,
        vault_id.as_ref(),
        FEE_POSITION_OWNER_SEED,
        &[ctx.bumps.fee_position_owner],
    ];
    let signer = &[&seeds[..]];
    
    // Prepare CPI accounts
    let cpi_accounts = ClaimFee {
        position: ctx.accounts.position.to_account_info(),
        lb_pair: ctx.accounts.lb_pair.to_account_info(),
        bin_array_lower: ctx.accounts.bin_array_lower.to_account_info(),
        bin_array_upper: ctx.accounts.bin_array_upper.to_account_info(),
        sender: ctx.accounts.fee_position_owner.to_account_info(),
        reserve_x: ctx.accounts.reserve_x.to_account_info(),
        reserve_y: ctx.accounts.reserve_y.to_account_info(),
        token_x_mint: ctx.accounts.token_x_mint.to_account_info(),
        token_y_mint: ctx.accounts.token_y_mint.to_account_info(),
        user_token_x: ctx.accounts.treasury_x.to_account_info(),
        user_token_y: ctx.accounts.treasury_y.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
        event_authority: ctx.accounts.event_authority.to_account_info(),
        program: ctx.accounts.dlmm_program.to_account_info(),
    };
    
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.dlmm_program.to_account_info(),
        cpi_accounts,
        signer,
    );
    
    // Claim fees
    let (amount_x, amount_y) = claim_fee(cpi_ctx)?;
    
    // Verify only quote fees were claimed
    let quote_is_x = ctx.accounts.token_x_mint.key() == ctx.accounts.vault.quote_mint;
    
    if quote_is_x {
        require!(amount_y == 0, FeeRouterError::BaseFeesNotAllowed);
        Ok(amount_x)
    } else {
        require!(amount_x == 0, FeeRouterError::BaseFeesNotAllowed);
        Ok(amount_y)
    }
}
```

## 4. Reading Streamflow Vesting Data

Integration with Streamflow to read locked amounts:

```typescript
import { Stream } from '@streamflow/stream';

async function getLockedAmount(
  streamPubkey: PublicKey,
  currentTimestamp: number
): Promise<BN> {
  // Fetch stream account
  const streamAccount = await Stream.fetch(
    connection,
    streamPubkey
  );
  
  if (!streamAccount) {
    return new BN(0);
  }
  
  // Calculate vested and locked amounts
  const totalAmount = streamAccount.depositedAmount;
  const startTime = streamAccount.startTime.toNumber();
  const endTime = streamAccount.endTime.toNumber();
  
  if (currentTimestamp < startTime) {
    // Not started yet, everything is locked
    return totalAmount;
  }
  
  if (currentTimestamp >= endTime) {
    // Fully vested, nothing locked
    return new BN(0);
  }
  
  // Linear vesting calculation
  const elapsed = currentTimestamp - startTime;
  const duration = endTime - startTime;
  const vestedAmount = totalAmount
    .mul(new BN(elapsed))
    .div(new BN(duration));
  
  const lockedAmount = totalAmount.sub(vestedAmount);
  
  // Account for already withdrawn amounts
  const withdrawnAmount = streamAccount.withdrawnAmount;
  const effectivelyLocked = lockedAmount.add(withdrawnAmount);
  
  return effectivelyLocked.gt(totalAmount) 
    ? totalAmount 
    : effectivelyLocked;
}
```

## 5. Complete Distribution Flow

Here's a complete example of the distribution process:

```typescript
import { Program, BN } from '@coral-xyz/anchor';
import { PublicKey, Transaction } from '@solana/web3.js';

class FeeDistributor {
  constructor(
    private program: Program,
    private vaultId: Buffer
  ) {}
  
  async performDailyDistribution() {
    // 1. Check if 24h have passed
    const distState = await this.getDistributionState();
    const now = Date.now() / 1000;
    
    if (now < distState.lastDistributionTs + 86400) {
      throw new Error('24h window not reached');
    }
    
    // 2. Get all investor pages
    const investorPages = await this.getInvestorPages();
    
    // 3. Process each page
    for (let page = 0; page < investorPages.length; page++) {
      const isLastPage = page === investorPages.length - 1;
      
      await this.distributeFeesForPage(
        page,
        investorPages[page],
        isLastPage
      );
    }
  }
  
  private async distributeFeesForPage(
    pageNumber: number,
    investors: InvestorData[],
    isLastPage: boolean
  ) {
    // Prepare remaining accounts
    const remainingAccounts = [];
    
    for (const investor of investors) {
      // Add investor's quote token account
      remainingAccounts.push({
        pubkey: investor.quoteAccount,
        isWritable: true,
        isSigner: false,
      });
      
      // Add investor's stream account
      remainingAccounts.push({
        pubkey: investor.streamPubkey,
        isWritable: false,
        isSigner: false,
      });
    }
    
    // Call distribute instruction
    const tx = await this.program.methods
      .distributeFees(
        Array.from(this.vaultId),
        pageNumber,
        isLastPage
      )
      .accounts({
        // ... main accounts
      })
      .remainingAccounts(remainingAccounts)
      .rpc();
    
    console.log(`Page ${pageNumber} distributed:`, tx);
  }
}
```

## 6. Monitoring and Analytics

Track distribution events:

```typescript
import { Program } from '@coral-xyz/anchor';

async function monitorDistributions(program: Program) {
  // Subscribe to QuoteFeesClaimed events
  program.addEventListener(
    'QuoteFeesClaimed',
    (event, slot) => {
      console.log('Fees claimed:', {
        vault: Buffer.from(event.vaultId).toString('hex'),
        amount: event.amountClaimed.toString(),
        day: event.distributionDay.toString(),
        timestamp: new Date(event.timestamp.toNumber() * 1000),
      });
    }
  );
  
  // Subscribe to InvestorPayout events
  program.addEventListener(
    'InvestorPayout',
    (event, slot) => {
      console.log('Investor payout:', {
        investor: event.investor.toBase58(),
        amount: event.amount.toString(),
        weight: event.weight.toString(),
        locked: event.lockedAmount.toString(),
      });
    }
  );
  
  // Subscribe to CreatorPayoutDayClosed events
  program.addEventListener(
    'CreatorPayoutDayClosed',
    (event, slot) => {
      console.log('Day closed:', {
        creatorPayout: event.creatorPayout.toString(),
        investorTotal: event.totalDistributedToInvestors.toString(),
        day: event.distributionDay.toString(),
      });
    }
  );
}
```

## 7. Error Recovery and Edge Cases

Handle various edge cases:

```typescript
async function robustDistribution() {
  try {
    await performDailyDistribution();
  } catch (error) {
    if (error.message.includes('DistributionWindowNotReached')) {
      // Wait until next window
      const timeToWait = calculateTimeToNextWindow();
      console.log(`Waiting ${timeToWait}ms until next window`);
      return;
    }
    
    if (error.message.includes('InvalidPageNumber')) {
      // Resume from correct page
      const currentPage = await getCurrentPage();
      await resumeFromPage(currentPage);
      return;
    }
    
    if (error.message.includes('BaseFeesNotAllowed')) {
      // Critical: base fees detected
      await alertAdministrators();
      throw error;
    }
    
    // Unknown error
    console.error('Distribution failed:', error);
    throw error;
  }
}
```

## 8. Testing Quote-Only Enforcement

Comprehensive test for quote-only validation:

```rust
#[test]
fn test_quote_only_enforcement() {
    // Setup pool with known configuration
    let pool = setup_test_pool();
    
    // Create position that would accrue base fees
    let bad_position = create_position(
        pool,
        INVALID_TICK_LOWER,
        INVALID_TICK_UPPER,
    );
    
    // Attempt to claim fees
    let result = claim_fees(bad_position);
    
    // Should fail if base fees present
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        FeeRouterError::BaseFeesNotAllowed
    );
    
    // Create correct quote-only position
    let good_position = create_quote_only_position(pool);
    
    // Should succeed with only quote fees
    let (quote_fees, base_fees) = claim_fees(good_position).unwrap();
    assert!(quote_fees > 0);
    assert_eq!(base_fees, 0);
}
```

## 9. Production Deployment Script

```bash
#!/bin/bash

# Deploy script for mainnet

set -e

echo "Building program..."
anchor build

echo "Running tests..."
anchor test

echo "Verifying IDL..."
anchor idl parse -f target/idl/dlmm_fee_router.json

echo "Deploying to mainnet..."
anchor deploy \
  --provider.cluster mainnet-beta \
  --provider.wallet ~/.config/solana/mainnet.json

echo "Upgrading authority to multisig..."
solana program set-upgrade-authority \
  FeeRouter11111111111111111111111111111111111 \
  --new-upgrade-authority $MULTISIG_ADDRESS

echo "Deployment complete!"
```

## Notes

- Always validate pool configuration before creating positions
- Monitor for pool parameter changes that might affect fee accrual
- Implement circuit breakers for abnormal fee amounts
- Consider implementing a timelock for parameter changes
- Set up monitoring alerts for failed distributions
- Maintain detailed logs of all distributions for audit purposes
