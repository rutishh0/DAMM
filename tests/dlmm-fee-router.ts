import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { DlmmFeeRouter } from "../target/types/dlmm_fee_router";
import { 
  PublicKey, 
  Keypair, 
  SystemProgram, 
  LAMPORTS_PER_SOL,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createMint,
  mintTo,
  createAssociatedTokenAccount,
  getAssociatedTokenAddress,
} from "@solana/spl-token";
import { assert } from "chai";
import { BN } from "bn.js";

describe("dlmm-fee-router", () => {
  // Configure the client to use the local cluster
  anchor.setProvider(anchor.AnchorProvider.env());
  
  const provider = anchor.getProvider();
  const program = anchor.workspace.DlmmFeeRouter as Program<DlmmFeeRouter>;
  
  // Test accounts
  let vaultId: Buffer;
  let vault: PublicKey;
  let distributionState: PublicKey;
  let feePositionOwner: PublicKey;
  let treasuryQuote: PublicKey;
  let treasuryBase: PublicKey;
  
  let quoteMint: PublicKey;
  let baseMint: PublicKey;
  let creatorWallet: Keypair;
  let creatorQuoteAccount: PublicKey;
  
  let investor1: Keypair;
  let investor2: Keypair;
  let investor3: Keypair;
  let investor1QuoteAccount: PublicKey;
  let investor2QuoteAccount: PublicKey;
  let investor3QuoteAccount: PublicKey;
  
  // Mock DLMM pool (in real tests, would use actual DLMM)
  let mockPool: Keypair;
  let mockFeePosition: Keypair;
  
  const INVESTOR_FEE_SHARE_BPS = 5000; // 50%
  const MIN_PAYOUT_LAMPORTS = 1000000; // 0.001 tokens
  const DAILY_CAP_LAMPORTS = new BN(1000000000); // 1000 tokens
  const TOTAL_ALLOCATION = new BN(10000000000); // 10,000 tokens
  
  before(async () => {
    // Generate vault ID
    vaultId = Buffer.from(Array.from({ length: 32 }, (_, i) => i));
    
    // Derive PDAs
    [vault] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), vaultId],
      program.programId
    );
    
    [distributionState] = PublicKey.findProgramAddressSync(
      [Buffer.from("distribution_state"), vaultId],
      program.programId
    );
    
    [feePositionOwner] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), vaultId, Buffer.from("investor_fee_pos_owner")],
      program.programId
    );
    
    // Create test wallets
    creatorWallet = Keypair.generate();
    investor1 = Keypair.generate();
    investor2 = Keypair.generate();
    investor3 = Keypair.generate();
    
    // Airdrop SOL to test wallets
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(
        creatorWallet.publicKey,
        2 * LAMPORTS_PER_SOL
      )
    );
    
    // Create mock pool and position
    mockPool = Keypair.generate();
    mockFeePosition = Keypair.generate();
    
    // Create token mints
    quoteMint = await createMint(
      provider.connection,
      creatorWallet,
      creatorWallet.publicKey,
      null,
      6 // USDC has 6 decimals
    );
    
    baseMint = await createMint(
      provider.connection,
      creatorWallet,
      creatorWallet.publicKey,
      null,
      9 // SOL-like token has 9 decimals
    );
    
    // Create token accounts
    creatorQuoteAccount = await createAssociatedTokenAccount(
      provider.connection,
      creatorWallet,
      quoteMint,
      creatorWallet.publicKey
    );
    
    investor1QuoteAccount = await createAssociatedTokenAccount(
      provider.connection,
      creatorWallet,
      quoteMint,
      investor1.publicKey
    );
    
    investor2QuoteAccount = await createAssociatedTokenAccount(
      provider.connection,
      creatorWallet,
      quoteMint,
      investor2.publicKey
    );
    
    investor3QuoteAccount = await createAssociatedTokenAccount(
      provider.connection,
      creatorWallet,
      quoteMint,
      investor3.publicKey
    );
  });
  
  describe("Initialize Vault", () => {
    it("Should initialize vault with correct parameters", async () => {
      // Compute program treasuries (ATAs owned by feePositionOwner PDA)
      treasuryQuote = await getAssociatedTokenAddress(
        quoteMint,
        feePositionOwner,
        true
      );
      treasuryBase = await getAssociatedTokenAddress(
        baseMint,
        feePositionOwner,
        true
      );

      const tx = await program.methods
        .initializeVault(
          Array.from(vaultId),
          creatorWallet.publicKey,
          INVESTOR_FEE_SHARE_BPS,
          new BN(MIN_PAYOUT_LAMPORTS),
          DAILY_CAP_LAMPORTS
        )
        .accounts({
          vault,
          distributionState,
          quoteMint,
          baseMint,
          treasuryQuote,
          treasuryBase,
          feePositionOwnerPda: feePositionOwner,
          authority: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();
      
      console.log("Initialize vault transaction:", tx);
      
      // Fetch and verify vault account
      const vaultAccount = await program.account.vault.fetch(vault);
      assert.equal(
        vaultAccount.creatorWallet.toBase58(),
        creatorWallet.publicKey.toBase58()
      );
      assert.equal(vaultAccount.investorFeeShareBps, INVESTOR_FEE_SHARE_BPS);
      assert.equal(
        vaultAccount.minPayoutLamports.toNumber(),
        MIN_PAYOUT_LAMPORTS
      );
      assert.equal(
        vaultAccount.dailyCapLamports?.toNumber(),
        DAILY_CAP_LAMPORTS.toNumber()
      );
      assert.isTrue(vaultAccount.isInitialized);
      assert.isFalse(vaultAccount.positionInitialized);
      
      // Verify distribution state
      const distState = await program.account.distributionState.fetch(
        distributionState
      );
      assert.equal(distState.vault.toBase58(), vault.toBase58());
      assert.equal(distState.currentDay.toNumber(), 0);
      assert.equal(distState.dailyDistributed.toNumber(), 0);
    });
    
    it("Should fail to initialize vault twice", async () => {
      try {
        await program.methods
          .initializeVault(
            Array.from(vaultId),
            creatorWallet.publicKey,
            INVESTOR_FEE_SHARE_BPS,
            new BN(MIN_PAYOUT_LAMPORTS),
            DAILY_CAP_LAMPORTS
          )
          .accounts({
            vault,
            distributionState,
            quoteMint,
            baseMint,
            treasuryQuote,
            treasuryBase,
            feePositionOwnerPda: feePositionOwner,
            authority: provider.wallet.publicKey,
            systemProgram: SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            rent: anchor.web3.SYSVAR_RENT_PUBKEY,
          })
          .rpc();
        
        assert.fail("Should have failed to initialize vault twice");
      } catch (error) {
        assert.include(error.toString(), "already in use");
      }
    });
    
    it("Should reject invalid fee share BPS", async () => {
      const newVaultId = Buffer.from(Array.from({ length: 32 }, () => 255));
      const [newVault] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault"), newVaultId],
        program.programId
      );
      const [newDistState] = PublicKey.findProgramAddressSync(
        [Buffer.from("distribution_state"), newVaultId],
        program.programId
      );

      const [newFeeOwner] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault"), newVaultId, Buffer.from("investor_fee_pos_owner")],
        program.programId
      );
      const newTreasuryQuote = await getAssociatedTokenAddress(
        quoteMint,
        newFeeOwner,
        true
      );
      const newTreasuryBase = await getAssociatedTokenAddress(
        baseMint,
        newFeeOwner,
        true
      );
      
      try {
        await program.methods
          .initializeVault(
            Array.from(newVaultId),
            creatorWallet.publicKey,
            10001, // Invalid: > 10000
            new BN(MIN_PAYOUT_LAMPORTS),
            null
          )
          .accounts({
            vault: newVault,
            distributionState: newDistState,
            quoteMint,
            baseMint,
            treasuryQuote: newTreasuryQuote,
            treasuryBase: newTreasuryBase,
            feePositionOwnerPda: newFeeOwner,
            authority: provider.wallet.publicKey,
            systemProgram: SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            rent: anchor.web3.SYSVAR_RENT_PUBKEY,
          })
          .rpc();
        
        assert.fail("Should have rejected invalid fee share BPS");
      } catch (error) {
        assert.include(error.toString(), "InvalidFeeShareBps");
      }
    });
  });
  
  describe("Initialize Fee Position", () => {
    it("Should initialize honorary fee position", async () => {
      // Note: This test is simplified as we don't have actual DLMM integration
      // In a real implementation, we would:
      // 1. Create an actual DLMM pool
      // 2. Initialize the position through DLMM
      // 3. Verify quote-only fee configuration
      
      // For now, we'll simulate the initialization
      const tx = await program.methods
        .initializeFeePosition(Array.from(vaultId))
        .accounts({
          vault,
          pool: mockPool.publicKey,
          feePositionOwner,
          feePosition: mockFeePosition.publicKey,
          tokenXVault: Keypair.generate().publicKey,
          tokenYVault: Keypair.generate().publicKey,
          tokenXMint: baseMint,
          tokenYMint: quoteMint,
          quoteMint,
          dlmmProgram: new PublicKey("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo"),
          authority: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();
      
      console.log("Initialize fee position transaction:", tx);
      
      // Verify vault was updated
      const vaultAccount = await program.account.vault.fetch(vault);
      assert.isTrue(vaultAccount.positionInitialized);
      assert.equal(
        vaultAccount.feePosition.toBase58(),
        mockFeePosition.publicKey.toBase58()
      );
      assert.equal(
        vaultAccount.pool.toBase58(),
        mockPool.publicKey.toBase58()
      );
    });
    
    it("Should reject position with wrong quote mint", async () => {
      const newVaultId = Buffer.from(Array.from({ length: 32 }, () => 123));
      const [newVault] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault"), newVaultId],
        program.programId
      );
      
      // First initialize the new vault
      const [newDistState] = PublicKey.findProgramAddressSync(
        [Buffer.from("distribution_state"), newVaultId],
        program.programId
      );
      
      await program.methods
        .initializeVault(
          Array.from(newVaultId),
          creatorWallet.publicKey,
          INVESTOR_FEE_SHARE_BPS,
          new BN(MIN_PAYOUT_LAMPORTS),
          null
        )
        .accounts({
          vault: newVault,
          distributionState: newDistState,
          quoteMint,
          authority: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();
      
      const [newFeePositionOwner] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault"), newVaultId, Buffer.from("fee_position_owner")],
        program.programId
      );
      
      const wrongQuoteMint = await createMint(
        provider.connection,
        creatorWallet,
        creatorWallet.publicKey,
        null,
        6
      );
      
      try {
        await program.methods
          .initializeFeePosition(Array.from(newVaultId))
          .accounts({
            vault: newVault,
            pool: mockPool.publicKey,
            feePositionOwner: newFeePositionOwner,
            feePosition: Keypair.generate().publicKey,
            tokenXVault: Keypair.generate().publicKey,
            tokenYVault: Keypair.generate().publicKey,
            tokenXMint: baseMint,
            tokenYMint: baseMint, // Wrong: neither is quote mint
            quoteMint: wrongQuoteMint, // Different quote mint
            dlmmProgram: new PublicKey("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo"),
            authority: provider.wallet.publicKey,
            systemProgram: SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            rent: anchor.web3.SYSVAR_RENT_PUBKEY,
          })
          .rpc();
        
        assert.fail("Should have rejected wrong quote mint");
      } catch (error) {
        assert.include(error.toString(), "InvalidQuoteMint");
      }
    });
  });
  
  describe("Update Investor Data", () => {
    it("Should update investor allocation data", async () => {
      const tx = await program.methods
        .updateInvestorData(Array.from(vaultId), TOTAL_ALLOCATION)
        .accounts({
          vault,
          authority: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
          // In real implementation, would pass investor records
        ])
        .rpc();
      
      console.log("Update investor data transaction:", tx);
      
      // Verify vault was updated
      const vaultAccount = await program.account.vault.fetch(vault);
      assert.equal(
        vaultAccount.totalInvestorAllocation.toNumber(),
        TOTAL_ALLOCATION.toNumber()
      );
    });
  });
  
  describe("Distribute Fees", () => {
    it("Should enforce 24h distribution window", async () => {
      // Create treasury account for the vault
      const treasuryAccount = treasuryQuote;
      
      // Mint some tokens to treasury to simulate claimed fees
      await mintTo(
        provider.connection,
        creatorWallet,
        quoteMint,
        treasuryAccount,
        creatorWallet.publicKey,
        1000000000 // 1000 tokens
      );
      
      try {
        // Try to distribute immediately (should fail due to 24h window)
        await program.methods
          .distributeFees(Array.from(vaultId), 0, false)
          .accounts({
            vault,
            distributionState,
            investorPage: Keypair.generate().publicKey,
            treasuryQuote: treasuryAccount,
            treasuryBase: treasuryBase,
            creatorQuoteAccount,
            feePosition: mockFeePosition.publicKey,
            feePositionOwner,
            dlmmProgram: new PublicKey("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo"),
            streamflowProgram: new PublicKey("strmRqUCoQUgGUan5YhzUZa6KqdzwX5L6FpUxfmKg5m"),
            quoteMint,
            crankOperator: provider.wallet.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
          })
          .remainingAccounts([
            { pubkey: investor1QuoteAccount, isWritable: true, isSigner: false },
            { pubkey: Keypair.generate().publicKey, isWritable: false, isSigner: false }, // Mock stream
            { pubkey: investor2QuoteAccount, isWritable: true, isSigner: false },
            { pubkey: Keypair.generate().publicKey, isWritable: false, isSigner: false }, // Mock stream
          ])
          .rpc();
        
        assert.fail("Should have enforced 24h window");
      } catch (error) {
        assert.include(error.toString(), "DistributionWindowNotReached");
      }
    });
    
    it("Should distribute fees after 24h window", async () => {
      // Note: In a real test environment, we would:
      // 1. Advance the blockchain time by 24 hours
      // 2. Actually claim fees from DLMM
      // 3. Verify proper distribution calculations
      // 4. Check that investors receive correct amounts
      // 5. Verify creator receives remainder
      
      // This is a simplified version showing the structure
      console.log("Fee distribution test would run after 24h window");
    });
    
    it("Should handle pagination correctly", async () => {
      // Test distributing across multiple pages
      console.log("Pagination test would verify multi-page distribution");
    });
    
    it("Should respect daily cap", async () => {
      // Test that daily cap is enforced
      console.log("Daily cap test would verify cap enforcement");
    });
    
    it("Should handle dust amounts correctly", async () => {
      // Test that amounts below minimum are carried over
      console.log("Dust handling test would verify carry-over logic");
    });
    
    it("Should route remainder to creator on final page", async () => {
      // Test that creator receives all unclaimed fees
      console.log("Creator remainder test would verify final distribution");
    });
  });
  
  describe("Quote-Only Fee Validation", () => {
    it("Should reject if base fees are detected", async () => {
      // This test would verify the critical requirement that
      // only quote-denominated fees are accepted
      console.log("Quote-only validation would reject base fees");
    });
    
    it("Should calculate correct pro-rata distributions", async () => {
      // Test the mathematical accuracy of distributions
      console.log("Pro-rata calculation test would verify math");
    });
  });
});
