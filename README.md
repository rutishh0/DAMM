# DLMM Fee Router - Permissionless Fee Routing for Meteora DLMM V2

## Overview

DLMM Fee Router is a Solana Anchor program that implements a permissionless fee distribution system for Meteora's Dynamic Liquidity Market Maker (DLMM) V2. The program creates and manages an "honorary" LP position that accrues fees exclusively in the quote token (typically USDC) and distributes them between investors and creators based on vesting schedules.

## Key Features

- **Quote-Only Fee Accrual**: Honorary position strictly collects fees in quote token only
- **Program-Owned Position**: Fee position is owned by a PDA for trustless operation
- **24-Hour Distribution Cycle**: Permissionless crank callable once per day
- **Pro-Rata Distribution**: Fees distributed based on still-locked token amounts
- **Pagination Support**: Handles large investor sets across multiple transactions
- **Streamflow Integration**: Reads vesting data directly from Streamflow streams
- **Configurable Parameters**: Customizable fee shares, minimums, and caps

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    DLMM Pool                         │
│  ┌───────────────────────────────────────────────┐  │
│  │          Honorary Fee Position (PDA)          │  │
│  │         (Accrues Quote-Only Fees)             │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
                          │
                          │ Claims Fees (24h)
                          ▼
        ┌──────────────────────────────────┐
        │     Treasury Quote (PDA)         │
        └──────────────────────────────────┘
                          │
                          │ Distributes
                          ▼
    ┌─────────────────────────────────────────┐
    │                                         │
    ▼                                         ▼
┌─────────────┐                    ┌──────────────┐
│  Investors  │                    │   Creator    │
│  (Pro-rata) │                    │ (Remainder)  │
└─────────────┘                    └──────────────┘
```

## Program Structure

### State Accounts

1. **Vault**: Main configuration and state storage
   - Creator wallet address
   - Fee position and pool references
   - Distribution parameters (fee share, minimums, caps)
   - Total investor allocation (Y0)

2. **DistributionState**: Tracks distribution progress
   - Last distribution timestamp
   - Current day and page numbers
   - Daily distributed amounts
   - Carry-over handling

3. **InvestorRecord**: Individual investor data
   - Stream pubkey reference
   - Initial allocation
   - Total fees received

4. **InvestorPage**: Paginated investor grouping
   - List of investor pubkeys
   - Cached total locked amounts

### Instructions

#### 1. Initialize Vault
```rust
pub fn initialize_vault(
    ctx: Context<InitializeVault>,
    vault_id: [u8; 32],
    creator_wallet: Pubkey,
    investor_fee_share_bps: u16,
    min_payout_lamports: u64,
    daily_cap_lamports: Option<u64>,
) -> Result<()>
```

Creates the vault configuration with distribution parameters.

#### 2. Initialize Fee Position
```rust
pub fn initialize_fee_position(
    ctx: Context<InitializeFeePosition>,
    vault_id: [u8; 32],
) -> Result<()>
```

Creates the honorary DLMM position that will accrue quote-only fees.

#### 3. Distribute Fees
```rust
pub fn distribute_fees(
    ctx: Context<DistributeFees>,
    vault_id: [u8; 32],
    page: u32,
    is_final_page: bool,
) -> Result<()>
```

Claims accumulated fees and distributes them to investors and creator.

#### 4. Update Investor Data
```rust
pub fn update_investor_data(
    ctx: Context<UpdateInvestorData>,
    vault_id: [u8; 32],
    total_allocation: u64,
) -> Result<()>
```

Updates investor allocation data and Y0 value.

## Distribution Formula

The program uses the following formula to calculate distributions:

```
Y0 = Total initial investor allocation
locked_total(t) = Sum of still-locked amounts at time t
f_locked(t) = locked_total(t) / Y0 (fraction locked)

eligible_investor_share_bps = min(
    configured_investor_fee_share_bps,
    floor(f_locked(t) × 10000)
)

investor_fee_quote = floor(
    claimed_fees × eligible_investor_share_bps / 10000
)

Individual payout:
weight_i(t) = locked_i(t) / locked_total(t)
payout_i = floor(investor_fee_quote × weight_i(t))
```

## Quote-Only Fee Enforcement

The program enforces quote-only fee accrual through multiple mechanisms:

1. **Position Validation**: Validates pool configuration during initialization
2. **Tick Range Selection**: Configures position to only accrue quote fees
3. **Claim Verification**: Rejects any claims that include base token fees
4. **Deterministic Failure**: Fails entire distribution if base fees detected

## Integration Guide

### Prerequisites

- Solana development environment
- Anchor CLI (>= 0.30.0)
- Node.js and Yarn/NPM
- Access to Meteora DLMM V2 pools
- Streamflow integration for vesting data

### Installation

```bash
# Clone the repository
git clone https://github.com/your-org/dlmm-fee-router
cd dlmm-fee-router

# Install dependencies
yarn install

# Build the program
anchor build
```

### Deployment

```bash
# Deploy to devnet
anchor deploy --provider.cluster devnet

# Deploy to mainnet
anchor deploy --provider.cluster mainnet-beta
```

### Usage Example

```typescript
import { Program, web3 } from '@coral-xyz/anchor';
import { DlmmFeeRouter } from './target/types/dlmm_fee_router';

// 1. Initialize Vault
const vaultId = Buffer.from(/* 32 bytes */);
await program.methods
  .initializeVault(
    Array.from(vaultId),
    creatorWallet,
    5000, // 50% to investors
    1_000_000, // 0.001 token minimum
    1_000_000_000 // 1000 token daily cap
  )
  .accounts({
    vault,
    distributionState,
    quoteMint,
    // ... other accounts
  })
  .rpc();

// 2. Initialize Fee Position
await program.methods
  .initializeFeePosition(Array.from(vaultId))
  .accounts({
    vault,
    pool: dlmmPool,
    feePosition,
    // ... other accounts
  })
  .rpc();

// 3. Distribute Fees (called by anyone after 24h)
await program.methods
  .distributeFees(
    Array.from(vaultId),
    0, // page number
    false // is final page
  )
  .accounts({
    vault,
    distributionState,
    treasuryQuote: treasuryQuote,
    treasuryBase: treasuryBase,
    // ... other accounts
  })
  .remainingAccounts([
    // Investor ATAs and stream accounts
  ])
  .rpc();
```

## Account Requirements

### PDAs (Program Derived Addresses)

| PDA | Seeds | Description |
|-----|-------|-------------|
| Vault | `["vault", vault_id]` | Main vault configuration |
| Distribution State | `["distribution_state", vault_id]` | Distribution tracking |
| Fee Position Owner | `["vault", vault_id, "investor_fee_pos_owner"]` | Position authority |
| Treasury Quote | `["treasury_quote", vault_id]` | Quote token treasury |
| Treasury Base | `["treasury_base", vault_id]` | Base mint treasury (should remain 0) |
| Investor Record | `["investor_record", vault_id, investor]` | Per-investor data |

### External Programs

- **DLMM Program**: `LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo`
- **Streamflow Program**: `strmRqUCoQUgGUan5YhzUZa6KqdzwX5L6FpUxfmKg5m`
- **Token Program**: `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`

## Events

The program emits the following events for monitoring and tracking:

```rust
pub struct VaultInitialized {
    pub vault_id: [u8; 32],
    pub creator: Pubkey,
    pub investor_fee_share_bps: u16,
    pub min_payout_lamports: u64,
    pub daily_cap_lamports: Option<u64>,
    pub timestamp: i64,
}

pub struct HonoraryPositionInitialized {
    pub vault_id: [u8; 32],
    pub position_pubkey: Pubkey,
    pub pool_pubkey: Pubkey,
    pub quote_mint: Pubkey,
    pub timestamp: i64,
}

pub struct QuoteFeesClaimed {
    pub vault_id: [u8; 32],
    pub amount_claimed: u64,
    pub timestamp: i64,
    pub distribution_day: u64,
}

pub struct InvestorPayoutPage {
    pub vault_id: [u8; 32],
    pub page: u32,
    pub total_payout: u64,
    pub investor_count: u32,
    pub timestamp: i64,
}

pub struct CreatorPayoutDayClosed {
    pub vault_id: [u8; 32],
    pub creator_payout: u64,
    pub total_distributed_to_investors: u64,
    pub distribution_day: u64,
    pub timestamp: i64,
}
```

## Error Codes

| Error | Description |
|-------|-------------|
| `InvalidFeeShareBps` | Fee share exceeds 10000 basis points |
| `DistributionWindowNotReached` | 24-hour window not elapsed |
| `BaseFeesNotAllowed` | Position would accrue base token fees |
| `InvalidPoolConfiguration` | Pool configuration invalid |
| `MathOverflow` | Arithmetic overflow |
| `PayoutBelowMinimum` | Payout below dust threshold |
| `DailyCapExceeded` | Daily distribution cap exceeded |
| `InvalidPageNumber` | Page number mismatch |
| `DistributionAlreadyCompleted` | Distribution already done for day |
| `InvalidQuoteMint` | Quote mint mismatch |
| `NoFeesToClaim` | No fees available to claim |
| `InvalidInvestorData` | Invalid investor data provided |

## Testing

Run the test suite:

```bash
# Run all tests
anchor test

# Run specific test file
anchor test -- --grep "Initialize Vault"

# Run with verbose output
anchor test -- --reporter spec
```

## Security Considerations

1. **Quote-Only Enforcement**: The program strictly enforces quote-only fee accrual
2. **PDA Ownership**: All critical accounts owned by PDAs for trustless operation
3. **Time-Gated Operations**: 24-hour distribution window prevents manipulation
4. **Overflow Protection**: All arithmetic operations checked for overflow
5. **Pagination Safety**: Idempotent pagination prevents double-spending
6. **Access Control**: Only authorized accounts can initialize vault

## Mainnet Deployment Checklist

- [ ] Complete audit of smart contract code
- [ ] Verify quote-only fee enforcement logic
- [ ] Test with actual DLMM pools on devnet
- [ ] Validate Streamflow integration
- [ ] Test pagination with large investor sets
- [ ] Verify mathematical accuracy of distributions
- [ ] Test edge cases (dust, caps, empty pools)
- [ ] Deploy with multisig authority
- [ ] Monitor initial distributions
- [ ] Set up event monitoring and alerts

## Future Enhancements

- Dynamic fee share based on market conditions
- Multiple pool support per vault
- Compound fee claiming strategies
- Advanced distribution schedules
- Cross-chain fee routing
- DAO governance for parameters

## License

MIT License - See LICENSE file for details

## Support

For questions or issues:
- Telegram: [@algopapi](https://t.me/algopapi)
- GitHub Issues: [Create an issue](https://github.com/your-org/dlmm-fee-router/issues)

## Acknowledgments

- Star Technologies for sponsoring development
- Meteora Protocol for DLMM V2
- Streamflow for vesting infrastructure
- Solana Foundation for blockchain infrastructure
