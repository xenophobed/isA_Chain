use crate::helpers::addr;
use isa_chain_core::{
    constants::PROTOCOL_FEE_PERCENT, ProtocolTreasury, ServiceType, SettlementEngine,
    SettlementStatus,
};

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
