//! Smoke / integration tests for the isA_Chain core blockchain crate.
//!
//! These tests exercise end-to-end flows across all major subsystems:
//! token economy, staking, credit system, subnet economy, agent wallets,
//! settlement, treasury, genesis config, payment channels, and the top-level
//! Blockchain struct.
//!
//! Run with:
//!   cargo test --manifest-path core/blockchain/Cargo.toml --test smoke_test 2>&1 | tail -30

use isa_chain_core::{
    // Token
    TokenState,
    // Staking
    StakingVault,
    // Credits
    CreditSystem,
    DEFAULT_CREDIT_PRICE_USD,
    DEFAULT_MIN_PURCHASE,
    // Subnet
    SubnetRegistry,
    SubnetId,
    SubnetStatus,
    ProviderSubnetStatus,
    // Agent wallet
    AgentWalletFactory,
    AgentWalletError,
    // Settlement
    SettlementEngine,
    ServiceType,
    SettlementStatus,
    // Treasury
    ProtocolTreasury,
    // Genesis
    GenesisConfig,
    // Payment channel
    ChannelManager,
    ChannelUpdate,
    ChannelStatus,
    // Blockchain
    Blockchain,
    // Types
    Address,
    Hash,
    // Constants
    constants::{
        INITIAL_SUPPLY,
        VALIDATOR_MIN_STAKE,
        PROTOCOL_FEE_PERCENT,
        MAIN_CHAIN_ID,
    },
};

// ============================================================================
// Shared test helpers
// ============================================================================

fn addr(byte: u8) -> Address {
    Address::from([byte; 20])
}

fn make_hash(seed: u8) -> Hash {
    Hash::new([seed; 32])
}

// ============================================================================
// 1. Token Economy Flow
// ============================================================================

mod token_economy {
    use super::*;

    /// Full token lifecycle: genesis supply → authorize → mint → burn → accounting.
    #[test]
    fn token_economy_flow() {
        let admin = addr(0xAD);
        let minter = addr(0xB0);
        let burner = addr(0xB1);

        // 1. Create token state with 1B ISA genesis supply.
        let mut ts = TokenState::new(INITIAL_SUPPLY, admin);
        assert_eq!(ts.get_total_supply(), INITIAL_SUPPLY);
        assert_eq!(ts.get_total_minted(), 0);
        assert_eq!(ts.get_total_burned(), 0);

        // 2. Authorize minter and burner.
        ts.authorize_minter(minter, &admin).unwrap();
        ts.authorize_burner(burner, &admin).unwrap();
        assert!(ts.is_authorized_minter(&minter));
        assert!(ts.is_authorized_burner(&burner));

        // 3. Mint tokens to user.
        let mint_amount: u128 = 1_000_000_000_000_000_000_000; // 1000 ISA
        ts.mint(mint_amount, &minter).unwrap();
        assert_eq!(ts.get_total_supply(), INITIAL_SUPPLY + mint_amount);
        assert_eq!(ts.get_total_minted(), mint_amount);

        // 4. Burn tokens (settlement).
        let burn_amount: u128 = 500_000_000_000_000_000_000; // 500 ISA
        ts.burn(burn_amount, &burner).unwrap();
        assert_eq!(
            ts.get_total_supply(),
            INITIAL_SUPPLY + mint_amount - burn_amount
        );
        assert_eq!(ts.get_total_burned(), burn_amount);

        // 5. Supply info snapshot is consistent.
        let info = ts.get_supply_info();
        assert_eq!(info.total_supply, ts.get_total_supply());
        assert_eq!(info.total_minted, mint_amount);
        assert_eq!(info.total_burned, burn_amount);
        assert_eq!(info.circulating_supply, info.total_supply);
    }
}

// ============================================================================
// 2. Staking Flow
// ============================================================================

mod staking_flow {
    use super::*;

    #[test]
    fn staking_full_lifecycle() {
        let provider = addr(0x01);
        let mut vault = StakingVault::default_vault(); // 32k ISA min, 100-block unbond

        // 1. Stake above the minimum.
        vault.stake(provider, VALIDATOR_MIN_STAKE, 1).unwrap();
        assert!(vault.is_staked(&provider));
        assert_eq!(vault.get_total_staked(), VALIDATOR_MIN_STAKE);

        let entry = vault.get_stake(&provider).unwrap();
        assert_eq!(entry.amount, VALIDATOR_MIN_STAKE);

        // 2. Begin unstaking half.
        let unstake_amount = VALIDATOR_MIN_STAKE / 2;
        vault.begin_unstake(&provider, unstake_amount, 100).unwrap();
        assert_eq!(vault.get_total_staked(), VALIDATOR_MIN_STAKE - unstake_amount);

        let entry_after = vault.get_stake(&provider).unwrap();
        assert_eq!(entry_after.unbonding.len(), 1);
        assert_eq!(entry_after.unbonding[0].amount, unstake_amount);
        assert_eq!(entry_after.unbonding[0].completion_height, 200); // 100 + 100

        // 3. Trying to complete before period ends returns 0.
        let early = vault.complete_unbonding(&provider, 150);
        assert_eq!(early, 0);

        // 4. Complete unbonding after period.
        let released = vault.complete_unbonding(&provider, 200);
        assert_eq!(released, unstake_amount);
        assert!(vault.get_stake(&provider).unwrap().unbonding.is_empty());

        // 5. Slash the remaining active stake (10% = 1000 bps).
        let slashed = vault.slash(&provider, 1000).unwrap();
        let remaining_before_slash = VALIDATOR_MIN_STAKE - unstake_amount;
        let expected_slash = remaining_before_slash / 10;
        assert_eq!(slashed, expected_slash);
        assert_eq!(
            vault.get_stake(&provider).unwrap().amount,
            remaining_before_slash - expected_slash
        );
    }
}

// ============================================================================
// 3. Credit System Flow
// ============================================================================

mod credit_system_flow {
    use super::*;

    /// ISA price: $0.50 = 500_000 micro-USD
    const ISA_PRICE_USD: u128 = 500_000;

    #[test]
    fn credit_purchase_and_spend() {
        let admin = addr(0xAD);
        let user = addr(0xC0);

        // 1. Create CreditSystem with $0.00001 per credit.
        let mut cs = CreditSystem::new(DEFAULT_CREDIT_PRICE_USD, DEFAULT_MIN_PURCHASE, admin);
        assert_eq!(cs.credit_price_usd, DEFAULT_CREDIT_PRICE_USD);

        // 2. Purchase credits: 1 ISA (1_000_000 micro-ISA) at $0.50
        //    credits = (1_000_000 * 500_000) / (100 * 1_000_000) = 5_000
        let isa_amount: u128 = 1_000_000;
        let credits = cs
            .purchase_credits(user, isa_amount, ISA_PRICE_USD, 10)
            .unwrap();
        assert_eq!(credits, 5_000, "1 ISA at $0.50 should yield 5_000 credits");
        assert_eq!(cs.get_balance(&user), 5_000);

        // 3. Spend some credits.
        let spend = 2_500;
        cs.spend_credits(&user, spend).unwrap();
        assert_eq!(cs.get_balance(&user), 2_500);

        // 4. Verify total accounting.
        assert_eq!(cs.total_credits_issued, 5_000);
        assert_eq!(cs.total_credits_burned, 2_500);
        assert_eq!(cs.total_credits_in_circulation(), 2_500);

        let acct = cs.get_account(&user).unwrap();
        assert_eq!(acct.total_credits_purchased, 5_000);
        assert_eq!(acct.total_credits_spent, 2_500);
    }

    #[test]
    fn credit_conversion_math() {
        // Verify the key math from the spec comment:
        // "1 ISA at $0.50 = 5000 credits"
        let credits = CreditSystem::credits_for_isa(1_000_000, ISA_PRICE_USD);
        assert_eq!(credits, 5_000);
    }
}

// ============================================================================
// 4. Subnet Economy Flow
// ============================================================================

mod subnet_economy_flow {
    use super::*;

    const ISA: u128 = 1_000_000_000_000_000_000; // 1 ISA in wei (used in model_min lookup)

    #[test]
    fn subnet_initialization_and_provider_registration() {
        let admin = addr(0xAD);
        let provider = addr(0x01);

        // 1. Create SubnetRegistry and initialize 6 subnets.
        let mut registry = SubnetRegistry::new(admin);
        registry.initialize_default_subnets(1).unwrap();
        assert_eq!(registry.subnet_count(), 6);

        // 2. Verify emission weights sum to 10,000.
        let weights = registry.get_emission_weights();
        let total_weight: u32 = weights.values().sum();
        assert_eq!(total_weight, 10_000, "emission weights must sum to 10_000 bps");

        // 3. Verify all 6 known subnets are present.
        assert!(registry.get_subnet(&SubnetId::Model).is_some());
        assert!(registry.get_subnet(&SubnetId::Tools).is_some());
        assert!(registry.get_subnet(&SubnetId::Compute).is_some());
        assert!(registry.get_subnet(&SubnetId::Storage).is_some());
        assert!(registry.get_subnet(&SubnetId::Agent).is_some());
        assert!(registry.get_subnet(&SubnetId::Market).is_some());

        // 4. Verify all subnet fee rates are 250 bps (2.5%).
        for subnet_id in &[
            SubnetId::Model,
            SubnetId::Tools,
            SubnetId::Compute,
            SubnetId::Storage,
            SubnetId::Agent,
            SubnetId::Market,
        ] {
            let config = registry.get_subnet(subnet_id).unwrap();
            assert_eq!(
                config.fee_rate_bps, 250,
                "subnet {:?} fee_rate_bps should be 250",
                subnet_id
            );
            assert_eq!(config.status, SubnetStatus::Active);
        }

        // 5. Register a provider in the Model subnet.
        let model_min = registry.get_subnet(&SubnetId::Model).unwrap().min_provider_stake;
        let _ = ISA; // reference to silence dead_code lint
        registry
            .register_provider(SubnetId::Model, provider, model_min, 10)
            .unwrap();

        let p = registry.get_provider(&SubnetId::Model, &provider).unwrap();
        assert_eq!(p.address, provider);
        assert_eq!(p.stake, model_min);
        assert_eq!(p.status, ProviderSubnetStatus::Active);
        assert_eq!(registry.total_providers(), 1);
    }
}

// ============================================================================
// 5. Agent Wallet Flow
// ============================================================================

mod agent_wallet_flow {
    use super::*;

    const ONE_ISA: u128 = 1_000_000_000_000_000_000;

    #[test]
    fn agent_wallet_lifecycle() {
        let owner = addr(0xAA);
        let agent_id = make_hash(0x01);

        let daily_limit = ONE_ISA;
        let monthly_limit = ONE_ISA * 20;
        let spending_limit = ONE_ISA * 100;

        let mut factory = AgentWalletFactory::new(spending_limit, 1000);

        // 1. Create wallet with daily/monthly limits and allowed_subnets.
        let wallet_addr = factory
            .create_wallet(
                agent_id,
                owner,
                spending_limit,
                1000,
                0,
                daily_limit,
                monthly_limit,
            )
            .unwrap();

        // Set allowed subnets to Model + Tools only.
        factory
            .get_wallet_mut(&wallet_addr)
            .unwrap()
            .allowed_subnets = Some(vec![SubnetId::Model, SubnetId::Tools]);

        // 2. Deposit funds.
        let deposit_amount = ONE_ISA * 10;
        factory.deposit(&wallet_addr, deposit_amount).unwrap();
        assert_eq!(factory.get_wallet(&wallet_addr).unwrap().balance, deposit_amount);

        // 3. Spend within daily limit on an allowed subnet — should succeed.
        let small_spend = ONE_ISA / 2;
        factory
            .spend(&wallet_addr, small_spend, 0, Some(SubnetId::Model))
            .unwrap();
        assert_eq!(
            factory.get_wallet(&wallet_addr).unwrap().balance,
            deposit_amount - small_spend
        );

        // 4. Spend exceeding daily limit — should fail.
        // Already spent small_spend; daily_limit = ONE_ISA; remaining = ONE_ISA/2.
        let too_much = daily_limit; // would push daily_spent over limit
        let result = factory.spend(&wallet_addr, too_much, 0, Some(SubnetId::Model));
        assert!(
            matches!(result, Err(AgentWalletError::DailyLimitExceeded { .. })),
            "expected DailyLimitExceeded, got {:?}",
            result
        );

        // 5. Spend on a disallowed subnet — should fail.
        let result2 = factory.spend(&wallet_addr, 100, 0, Some(SubnetId::Compute));
        assert_eq!(result2, Err(AgentWalletError::SubnetNotAllowed));
    }
}

// ============================================================================
// 6. Settlement Flow
// ============================================================================

mod settlement_flow {
    use super::*;

    #[test]
    fn settlement_fee_calculation_and_accounting() {
        let user = addr(0x11);
        let provider = addr(0x22);

        // 1. Create SettlementEngine with 2.5% fee.
        let mut engine = SettlementEngine::new(PROTOCOL_FEE_PERCENT);

        // 2. Settle a transaction.
        let gross: u128 = 1_000_000;
        let record = engine
            .settle(
                user,
                provider,
                gross,
                ServiceType::ModelInference,
                42,
                1_000_000,
            )
            .unwrap();

        // 3. Verify fee calculation: 2.5% of 1_000_000 = 25_000.
        assert_eq!(record.gross_amount, gross);
        assert_eq!(record.fee_amount, 25_000);
        assert_eq!(record.net_amount, 975_000);
        assert_eq!(record.fee_amount + record.net_amount, gross);
        assert_eq!(record.status, SettlementStatus::Completed);

        // 4. Verify engine totals.
        assert_eq!(engine.get_total_settled(), gross);
        assert_eq!(engine.get_total_fees(), 25_000);

        // 5. Verify net amount = gross - fee.
        let (fee, net) = engine.calculate_split(gross);
        assert_eq!(fee, 25_000);
        assert_eq!(net, 975_000);
        assert_eq!(fee + net, gross);
    }
}

// ============================================================================
// 7. Treasury Flow
// ============================================================================

mod treasury_flow {
    use super::*;

    #[test]
    fn treasury_collect_and_distribute_split() {
        let staker_a = addr(0x01);
        let staker_b = addr(0x02);
        let admin = addr(0xAD);

        // 1. Create ProtocolTreasury.
        let mut treasury = ProtocolTreasury::new(PROTOCOL_FEE_PERCENT, admin);

        // 2. Collect fees (direct deposit for simplicity).
        treasury.deposit(10_000).unwrap();
        assert_eq!(treasury.get_balance(), 10_000);

        // 3. Distribute with 60/40 split.
        //    40% to stakers, 60% retained.
        let stakers = vec![(staker_a, 1_000), (staker_b, 1_000)];
        let (staker_dist, treasury_retention) =
            treasury.distribute_with_split(&stakers, 1, &admin).unwrap();

        // 4. Verify staker share = 40% of balance.
        assert_eq!(staker_dist.total_amount, 4_000); // 40% of 10_000
        assert_eq!(treasury_retention.total_amount, 6_000); // 60% of 10_000

        // 5. Verify equal split between two equal stakers.
        assert_eq!(staker_dist.recipients.len(), 2);
        assert_eq!(staker_dist.recipients[0].1, 2_000);
        assert_eq!(staker_dist.recipients[1].1, 2_000);

        // 6. Treasury balance drops by staker portion only.
        assert_eq!(treasury.get_balance(), 6_000);
        assert_eq!(treasury.get_total_distributed(), 4_000);
    }
}

// ============================================================================
// 8. Genesis Config Validation
// ============================================================================

mod genesis_config {
    use super::*;

    #[test]
    fn default_mainnet_genesis_is_valid() {
        let config = GenesisConfig::default_mainnet();

        // 1. Validates successfully.
        config.validate().expect("mainnet genesis should be valid");

        // 2. Total allocated equals INITIAL_SUPPLY (1B ISA).
        assert_eq!(
            config.total_allocated(),
            INITIAL_SUPPLY,
            "total allocated must equal 1B ISA"
        );
        assert_eq!(config.initial_supply, INITIAL_SUPPLY);

        // 3. Verify allocation percentages match PRD (40/20/15/15/10).
        //    Treasury immediate = 15%.
        let treasury_expected = INITIAL_SUPPLY * 15 / 100;
        let treasury_alloc = config.allocations.iter().map(|a| a.amount).sum::<u128>();
        assert_eq!(treasury_alloc, treasury_expected, "treasury should be 15%");

        //    Vesting schedules: ecosystem(40) + team(20) + provider_incentives(15) + early_supporters(10).
        let vested_total: u128 = config.vesting_schedules.iter().map(|v| v.total_amount).sum();
        let expected_vested = INITIAL_SUPPLY * 85 / 100; // everything except 15% treasury
        assert_eq!(vested_total, expected_vested, "vested should be 85%");

        // 4. Chain ID is mainnet.
        assert_eq!(config.chain_id, MAIN_CHAIN_ID);
    }
}

// ============================================================================
// 9. Payment Channel Flow
// ============================================================================

mod payment_channel_flow {
    use super::*;

    const DEPOSIT: u128 = 10_000;
    const DISPUTE_PERIOD: u64 = 50;
    const DEFAULT_EXPIRY: u64 = 100_000;

    #[test]
    fn channel_open_update_close_finalize() {
        let sender = addr(0xAA);
        let receiver = addr(0xBB);

        let mut mgr = ChannelManager::new(DISPUTE_PERIOD, DEFAULT_EXPIRY);

        // 1. Open channel between two parties.
        let channel_id = mgr
            .open_channel(sender, receiver, DEPOSIT, 100)
            .unwrap();

        let ch = mgr.get_channel(&channel_id).unwrap();
        assert_eq!(ch.sender, sender);
        assert_eq!(ch.receiver, receiver);
        assert_eq!(ch.deposit, DEPOSIT);
        assert_eq!(ch.sender_balance, DEPOSIT);
        assert_eq!(ch.receiver_balance, 0);
        assert_eq!(ch.status, ChannelStatus::Open);

        // 2. Update state: transfer 3000 to receiver.
        let update = ChannelUpdate {
            channel_id,
            nonce: 1,
            sender_balance: 7_000,
            receiver_balance: 3_000,
        };
        mgr.update_channel(&channel_id, update).unwrap();

        let ch = mgr.get_channel(&channel_id).unwrap();
        assert_eq!(ch.nonce, 1);
        assert_eq!(ch.sender_balance, 7_000);
        assert_eq!(ch.receiver_balance, 3_000);

        // 3. Close channel.
        mgr.close_channel(&channel_id, &sender, 200).unwrap();
        assert_eq!(
            mgr.get_channel(&channel_id).unwrap().status,
            ChannelStatus::Closing { initiated_at: 200 }
        );

        // 4. Finalize after dispute period (200 + 50 = 250).
        let (refund, payout) = mgr.finalize_channel(&channel_id, 250).unwrap();

        // 5. Verify refund + payout = deposit.
        assert_eq!(refund, 7_000);
        assert_eq!(payout, 3_000);
        assert_eq!(refund + payout, DEPOSIT);
        assert_eq!(
            mgr.get_channel(&channel_id).unwrap().status,
            ChannelStatus::Closed
        );
    }

    #[test]
    fn channel_dispute_flow() {
        let sender = addr(0xAA);
        let receiver = addr(0xBB);
        let mut mgr = ChannelManager::new(DISPUTE_PERIOD, DEFAULT_EXPIRY);

        let channel_id = mgr.open_channel(sender, receiver, DEPOSIT, 100).unwrap();

        // Advance state to nonce 1.
        mgr.update_channel(
            &channel_id,
            ChannelUpdate {
                channel_id,
                nonce: 1,
                sender_balance: 6_000,
                receiver_balance: 4_000,
            },
        )
        .unwrap();

        // Sender tries to close at stale state.
        mgr.close_channel(&channel_id, &sender, 200).unwrap();

        // Receiver disputes with a newer nonce.
        mgr.dispute_channel(
            &channel_id,
            ChannelUpdate {
                channel_id,
                nonce: 2,
                sender_balance: 2_000,
                receiver_balance: 8_000,
            },
            &receiver,
            210,
        )
        .unwrap();

        assert_eq!(
            mgr.get_channel(&channel_id).unwrap().status,
            ChannelStatus::Disputed { disputed_at: 210 }
        );

        // Finalize after dispute period from dispute time (210 + 50 = 260).
        let (refund, payout) = mgr.finalize_channel(&channel_id, 260).unwrap();
        assert_eq!(refund, 2_000);
        assert_eq!(payout, 8_000);
        assert_eq!(refund + payout, DEPOSIT);
    }
}

// ============================================================================
// 10. Blockchain Integration
// ============================================================================

mod blockchain_integration {
    use super::*;

    #[test]
    fn blockchain_struct_accessible_fields() {
        let mut chain = Blockchain::new(MAIN_CHAIN_ID);

        // 1. Treasury and staking_vault are accessible.
        let _treasury = chain.treasury();
        let _vault = chain.staking_vault();

        // 2. token_state is accessible.
        assert_eq!(chain.token_state().get_total_supply(), INITIAL_SUPPLY);

        // 3. Mint tokens via blockchain (genesis-level, bypasses authority).
        let recipient = addr(0x42);
        let amount: u128 = 1_000_000_000_000_000_000_000;
        chain.mint(recipient, amount);
        assert_eq!(chain.get_balance(&recipient), amount);

        // 4. Authorize a minter via token_state_mut, then mint via mint_tokens.
        let admin = addr(0xAD);
        let minter = addr(0xB0);

        // First configure admin by creating a fresh token state — Blockchain uses Address::ZERO
        // as default admin, so we use the public token_state_mut to authorize.
        chain
            .token_state_mut()
            .authorize_minter(minter, &Address::ZERO)
            .unwrap();

        let recipient2 = addr(0x43);
        let mint_amount: u128 = 5_000_000_000_000_000_000_000;
        chain.mint_tokens(recipient2, mint_amount, &minter).unwrap();
        assert_eq!(chain.get_balance(&recipient2), mint_amount);
        assert_eq!(
            chain.token_state().get_total_supply(),
            INITIAL_SUPPLY + mint_amount
        );

        // 5. Build and produce a block (empty block is valid).
        let _block = chain.build_block(100).unwrap();
        let block_hash = chain.produce_block(100).unwrap();
        assert_ne!(block_hash, Hash::ZERO);
        assert_eq!(chain.get_height(), 1);
    }

    #[test]
    fn blockchain_height_and_genesis() {
        let chain = Blockchain::new(MAIN_CHAIN_ID);

        assert_eq!(chain.get_height(), 0);
        assert_ne!(chain.genesis_hash, Hash::ZERO);
        // Genesis block is retrievable.
        assert!(chain.get_block_by_height(0).is_some());
    }

    #[test]
    fn blockchain_burn_tokens() {
        let mut chain = Blockchain::new(MAIN_CHAIN_ID);

        let burner = addr(0xB1);
        let user = addr(0xC0);
        let amount: u128 = 10_000_000_000_000_000_000_000;

        // Fund the user directly.
        chain.mint(user, amount);

        // Authorize burner.
        chain
            .token_state_mut()
            .authorize_burner(burner, &Address::ZERO)
            .unwrap();

        // Burn half.
        let burn_amount = amount / 2;
        chain.burn_tokens(user, burn_amount, &burner).unwrap();

        assert_eq!(chain.get_balance(&user), amount - burn_amount);
        assert_eq!(chain.token_state().get_total_burned(), burn_amount);
        assert_eq!(
            chain.token_state().get_total_supply(),
            INITIAL_SUPPLY - burn_amount
        );
    }
}

// ============================================================================
// 11. Cross-cutting: Treasury fee collection via SettlementEngine
// ============================================================================

mod cross_cutting {
    use super::*;

    /// Verify that settlement fee math and treasury collection are consistent.
    #[test]
    fn settlement_and_treasury_fee_consistency() {
        let admin = addr(0xAD);
        let user = addr(0x11);
        let provider = addr(0x22);

        let mut engine = SettlementEngine::new(PROTOCOL_FEE_PERCENT);
        let mut treasury = ProtocolTreasury::new(PROTOCOL_FEE_PERCENT, admin);

        let gross: u128 = 4_000_000;

        let record = engine
            .settle(user, provider, gross, ServiceType::AgentRuntime, 10, 9_000)
            .unwrap();

        // Fee = 2.5% of 4_000_000 = 100_000.
        assert_eq!(record.fee_amount, 100_000);
        assert_eq!(record.net_amount, 3_900_000);

        // Route the fee into treasury.
        treasury.deposit(record.fee_amount).unwrap();
        assert_eq!(treasury.get_balance(), 100_000);
        assert_eq!(treasury.get_total_collected(), 100_000);
    }
}
