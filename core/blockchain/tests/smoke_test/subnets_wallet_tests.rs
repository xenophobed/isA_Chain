use crate::helpers::{addr, make_hash};
use isa_chain_core::{
    AgentWalletError, AgentWalletFactory, ProviderSubnetStatus, SubnetId, SubnetRegistry,
    SubnetStatus,
};

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
