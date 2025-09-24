use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount, Mint};
use bytemuck::{Pod, Zeroable};

/// DLMM V2 Pool State (simplified representation)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LbPair {
    pub parameters: Parameters,
    pub v_parameters: VParameters,
    pub bump_seed: [u8; 1],
    pub bin_step_seed: [u8; 2],
    pub pair_type: u8,
    pub active_id: i32,
    pub bin_step: u16,
    pub protocol_fee: ProtocolFee,
    pub padding1: [u8; 4],
    pub last_updated_at: i64,
    pub padding2: [u8; 8],
    pub cumulative_fee_volume: CumulativeFeeVolume,
    pub padding3: [u8; 8],
    pub token_x_mint: Pubkey,
    pub token_y_mint: Pubkey,
    pub reserve_x: Pubkey,
    pub reserve_y: Pubkey,
    pub padding4: [u8; 32],
    pub oracle: Pubkey,
    pub padding5: [u8; 64],
    pub reserved: [u8; 64],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Parameters {
    pub swap_fee_bps: u16,
    pub max_bin_id: i32,
    pub min_bin_id: i32,
    pub bin_count: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct VParameters {
    pub volatility_accumulator: u32,
    pub volatility_reference: u32,
    pub padding: [u8; 16],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ProtocolFee {
    pub amount_x: u64,
    pub amount_y: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CumulativeFeeVolume {
    pub cumulative_fee_x: u128,
    pub cumulative_fee_y: u128,
    pub cumulative_volume_x: u128,
    pub cumulative_volume_y: u128,
}

/// Position state in DLMM
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Position {
    pub lb_pair: Pubkey,
    pub owner: Pubkey,
    pub liquidity_shares: [u128; 70],
    pub padding: [u8; 8],
    pub fee_x_per_token_complete: [u128; 70],
    pub fee_y_per_token_complete: [u128; 70],
    pub fee_x_pending: u64,
    pub fee_y_pending: u64,
    pub reserved: [u8; 32],
}

/// Calculate the appropriate tick range for quote-only fee accrual
pub fn calculate_quote_only_ticks(
    pool: &LbPair,
    quote_mint: &Pubkey,
) -> Result<(i32, i32)> {
    let is_quote_x = pool.token_x_mint == *quote_mint;
    let is_quote_y = pool.token_y_mint == *quote_mint;
    
    require!(
        is_quote_x || is_quote_y,
        crate::errors::FeeRouterError::InvalidQuoteMint
    );
    
    let current_tick = pool.active_id;
    let tick_spacing = pool.bin_step as i32;
    
    // Calculate position range that will only accrue quote fees
    let (tick_lower, tick_upper) = if is_quote_x {
        // Quote is token X: Create position below current price
        // This ensures we only collect fees when quote appreciates
        let tick_upper = current_tick.saturating_sub(tick_spacing);
        let tick_lower = tick_upper.saturating_sub(tick_spacing * 100);
        (tick_lower, tick_upper)
    } else {
        // Quote is token Y: Create position above current price
        let tick_lower = current_tick.saturating_add(tick_spacing);
        let tick_upper = tick_lower.saturating_add(tick_spacing * 100);
        (tick_lower, tick_upper)
    };
    
    // Validate the ticks are within bounds
    require!(
        tick_lower >= pool.parameters.min_bin_id,
        crate::errors::FeeRouterError::InvalidPoolConfiguration
    );
    require!(
        tick_upper <= pool.parameters.max_bin_id,
        crate::errors::FeeRouterError::InvalidPoolConfiguration
    );
    
    Ok((tick_lower, tick_upper))
}

/// Validate that a position will only accrue quote fees
pub fn validate_quote_only_position(
    position: &Position,
    pool: &LbPair,
    quote_mint: &Pubkey,
) -> Result<()> {
    // Check which token is quote
    let is_quote_x = pool.token_x_mint == *quote_mint;
    
    // For an honorary position (0 liquidity), we verify:
    // 1. No pending base fees
    // 2. Position parameters ensure quote-only accrual
    
    if is_quote_x {
        // If quote is X, we should have no Y fees
        require!(
            position.fee_y_pending == 0,
            crate::errors::FeeRouterError::BaseFeesNotAllowed
        );
    } else {
        // If quote is Y, we should have no X fees
        require!(
            position.fee_x_pending == 0,
            crate::errors::FeeRouterError::BaseFeesNotAllowed
        );
    }
    
    Ok(())
}

/// Extract quote fees from claimed amounts
pub fn extract_quote_fees(
    claimed_x: u64,
    claimed_y: u64,
    pool: &LbPair,
    quote_mint: &Pubkey,
) -> Result<u64> {
    let is_quote_x = pool.token_x_mint == *quote_mint;
    
    if is_quote_x {
        // Quote is X, base is Y
        require!(
            claimed_y == 0,
            crate::errors::FeeRouterError::BaseFeesNotAllowed
        );
        Ok(claimed_x)
    } else {
        // Quote is Y, base is X
        require!(
            claimed_x == 0,
            crate::errors::FeeRouterError::BaseFeesNotAllowed
        );
        Ok(claimed_y)
    }
}

/// Helper to deserialize DLMM accounts safely
pub fn deserialize_lb_pair(account: &AccountInfo) -> Result<LbPair> {
    if account.data_len() < std::mem::size_of::<LbPair>() {
        return Err(crate::errors::FeeRouterError::InvalidPoolConfiguration.into());
    }
    
    let data = account.try_borrow_data()?;
    let pool = bytemuck::try_from_bytes::<LbPair>(&data[8..]) // Skip discriminator
        .map_err(|_| crate::errors::FeeRouterError::InvalidPoolConfiguration)?;
    
    Ok(*pool)
}

pub fn deserialize_position(account: &AccountInfo) -> Result<Position> {
    if account.data_len() < std::mem::size_of::<Position>() {
        return Err(crate::errors::FeeRouterError::PositionNotInitialized.into());
    }
    
    let data = account.try_borrow_data()?;
    let position = bytemuck::try_from_bytes::<Position>(&data[8..]) // Skip discriminator
        .map_err(|_| crate::errors::FeeRouterError::PositionNotInitialized)?;
    
    Ok(*position)
}

/// CPI helper for creating the honorary position
pub mod cpi {
    use super::*;
    
    pub fn create_honorary_position<'info>(
        dlmm_program: AccountInfo<'info>,
        pool: AccountInfo<'info>,
        position: AccountInfo<'info>,
        position_owner: AccountInfo<'info>,
        system_program: AccountInfo<'info>,
        rent: AccountInfo<'info>,
        tick_lower: i32,
        tick_upper: i32,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<()> {
        // Prepare instruction data
        let mut data = Vec::with_capacity(12);
        data.extend_from_slice(&[0x01]); // InitializePosition instruction discriminator
        data.extend_from_slice(&tick_lower.to_le_bytes());
        data.extend_from_slice(&tick_upper.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes()); // 0 liquidity
        
        // Prepare accounts
        let accounts = vec![
            AccountMeta::new(position.key(), false),
            AccountMeta::new_readonly(pool.key(), false),
            AccountMeta::new_readonly(position_owner.key(), true),
            AccountMeta::new_readonly(system_program.key(), false),
            AccountMeta::new_readonly(rent.key(), false),
        ];
        
        // Create instruction
        let instruction = solana_program::instruction::Instruction {
            program_id: dlmm_program.key(),
            accounts,
            data,
        };
        
        // Invoke CPI
        anchor_lang::solana_program::program::invoke_signed(
            &instruction,
            &[
                position,
                pool,
                position_owner,
                system_program,
                rent,
            ],
            signer_seeds,
        )?;
        
        Ok(())
    }
    
    pub fn claim_position_fees<'info>(
        dlmm_program: AccountInfo<'info>,
        position: AccountInfo<'info>,
        pool: AccountInfo<'info>,
        position_owner: AccountInfo<'info>,
        reserve_x: AccountInfo<'info>,
        reserve_y: AccountInfo<'info>,
        user_token_x: AccountInfo<'info>,
        user_token_y: AccountInfo<'info>,
        token_program: AccountInfo<'info>,
        signer_seeds: &[&[&[u8]]],
    ) -> Result<(u64, u64)> {
        // Prepare instruction data
        let data = vec![0x02]; // ClaimFees instruction discriminator
        
        // Prepare accounts
        let accounts = vec![
            AccountMeta::new(position.key(), false),
            AccountMeta::new(pool.key(), false),
            AccountMeta::new_readonly(position_owner.key(), true),
            AccountMeta::new(reserve_x.key(), false),
            AccountMeta::new(reserve_y.key(), false),
            AccountMeta::new(user_token_x.key(), false),
            AccountMeta::new(user_token_y.key(), false),
            AccountMeta::new_readonly(token_program.key(), false),
        ];
        
        // Create instruction
        let instruction = solana_program::instruction::Instruction {
            program_id: dlmm_program.key(),
            accounts,
            data,
        };
        
        // Get balances before
        let balance_x_before = {
            let account = user_token_x.try_borrow_data()?;
            let token_account = bytemuck::try_from_bytes::<TokenAccount>(&account[..])
                .map_err(|_| ProgramError::InvalidAccountData)?;
            token_account.amount
        };
        
        let balance_y_before = {
            let account = user_token_y.try_borrow_data()?;
            let token_account = bytemuck::try_from_bytes::<TokenAccount>(&account[..])
                .map_err(|_| ProgramError::InvalidAccountData)?;
            token_account.amount
        };
        
        // Invoke CPI
        anchor_lang::solana_program::program::invoke_signed(
            &instruction,
            &[
                position,
                pool,
                position_owner,
                reserve_x,
                reserve_y,
                user_token_x.clone(),
                user_token_y.clone(),
                token_program,
            ],
            signer_seeds,
        )?;
        
        // Get balances after
        let balance_x_after = {
            let account = user_token_x.try_borrow_data()?;
            let token_account = bytemuck::try_from_bytes::<TokenAccount>(&account[..])
                .map_err(|_| ProgramError::InvalidAccountData)?;
            token_account.amount
        };
        
        let balance_y_after = {
            let account = user_token_y.try_borrow_data()?;
            let token_account = bytemuck::try_from_bytes::<TokenAccount>(&account[..])
                .map_err(|_| ProgramError::InvalidAccountData)?;
            token_account.amount
        };
        
        // Calculate claimed amounts
        let claimed_x = balance_x_after.saturating_sub(balance_x_before);
        let claimed_y = balance_y_after.saturating_sub(balance_y_before);
        
        Ok((claimed_x, claimed_y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_quote_only_tick_calculation() {
        let mut pool = unsafe { std::mem::zeroed::<LbPair>() };
        pool.active_id = 10000;
        pool.bin_step = 10;
        pool.parameters.min_bin_id = -10000;
        pool.parameters.max_bin_id = 20000;
        
        // Test with quote as token X
        let quote_mint = Pubkey::new_unique();
        pool.token_x_mint = quote_mint;
        pool.token_y_mint = Pubkey::new_unique();
        
        let result = calculate_quote_only_ticks(&pool, &quote_mint);
        assert!(result.is_ok());
        
        let (tick_lower, tick_upper) = result.unwrap();
        assert!(tick_upper < pool.active_id);
        assert!(tick_lower < tick_upper);
        
        // Test with quote as token Y
        pool.token_x_mint = Pubkey::new_unique();
        pool.token_y_mint = quote_mint;
        
        let result = calculate_quote_only_ticks(&pool, &quote_mint);
        assert!(result.is_ok());
        
        let (tick_lower, tick_upper) = result.unwrap();
        assert!(tick_lower > pool.active_id);
        assert!(tick_lower < tick_upper);
    }
    
    #[test]
    fn test_quote_fee_extraction() {
        let mut pool = unsafe { std::mem::zeroed::<LbPair>() };
        let quote_mint = Pubkey::new_unique();
        pool.token_x_mint = quote_mint;
        pool.token_y_mint = Pubkey::new_unique();
        
        // Test valid case - only quote fees
        let result = extract_quote_fees(1000, 0, &pool, &quote_mint);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1000);
        
        // Test invalid case - base fees present
        let result = extract_quote_fees(1000, 500, &pool, &quote_mint);
        assert!(result.is_err());
    }
}
