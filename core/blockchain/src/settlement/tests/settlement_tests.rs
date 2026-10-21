use super::helpers::*;
use crate::settlement::types::{ServiceType, SettlementError, SettlementStatus};
use crate::types::{Address, Hash};

// ---- Core settlement tests -----------------------------------------

#[test]
fn test_settle_success() {
    let mut e = engine();
    let result = e.settle(
        user(),
        provider(),
        GROSS,
        ServiceType::ModelInference,
        1,
        1_000,
    );
    assert!(result.is_ok());
    let rec = result.unwrap();
    assert_eq!(rec.user, user());
    assert_eq!(rec.provider, provider());
    assert_eq!(rec.gross_amount, GROSS);
    assert_eq!(rec.status, SettlementStatus::Completed);
}

#[test]
fn test_settle_zero_amount_fails() {
    let mut e = engine();
    let result = e.settle(user(), provider(), 0, ServiceType::ModelInference, 1, 1_000);
    assert!(matches!(result, Err(SettlementError::InvalidAmount)));
}

#[test]
fn test_settle_same_user_provider_fails() {
    let mut e = engine();
    let result = e.settle(user(), user(), GROSS, ServiceType::ModelInference, 1, 1_000);
    assert!(matches!(result, Err(SettlementError::SameUserAndProvider)));
}

// ---- Fee calculation tests -----------------------------------------

#[test]
fn test_fee_calculation() {
    let e = engine(); // 250 bps = 2.5%
    let (fee, net) = e.calculate_split(1_000_000);
    // fee = 1_000_000 * 250 / 10_000 = 25_000
    assert_eq!(fee, 25_000);
    assert_eq!(net, 975_000);
    assert_eq!(fee + net, 1_000_000);
}

#[test]
fn test_fee_calculation_small_amount() {
    let e = engine();
    // 1 unit → fee rounds down to 0, net = 1
    let (fee, net) = e.calculate_split(1);
    assert_eq!(fee, 0);
    assert_eq!(net, 1);

    // 100 units → fee = 100 * 250 / 10_000 = 2 (floor)
    let (fee, net) = e.calculate_split(100);
    assert_eq!(fee, 2);
    assert_eq!(net, 98);
}

// ---- Batch settlement tests ----------------------------------------

#[test]
fn test_batch_settle() {
    let mut e = engine();
    let batch = vec![
        (user(), provider(), 500_000u128, ServiceType::ModelInference),
        (user(), provider2(), 300_000u128, ServiceType::ToolExecution),
    ];
    let results = e.batch_settle(batch, 5, 5_000);
    assert_eq!(results.len(), 2);
    assert!(results[0].is_ok());
    assert!(results[1].is_ok());
    assert_eq!(e.records.len(), 2);
}

#[test]
fn test_batch_settle_mixed_results() {
    let mut e = engine();
    let batch = vec![
        (user(), provider(), 500_000u128, ServiceType::ModelInference),
        // zero amount — will fail
        (user(), provider2(), 0u128, ServiceType::ToolExecution),
        (user(), provider(), 200_000u128, ServiceType::Storage),
    ];
    let results = e.batch_settle(batch, 10, 10_000);
    assert_eq!(results.len(), 3);
    assert!(results[0].is_ok());
    assert!(matches!(results[1], Err(SettlementError::InvalidAmount)));
    assert!(results[2].is_ok());
    // Only 2 successful records persisted
    assert_eq!(e.records.len(), 2);
}

// ---- Index / query tests -------------------------------------------

#[test]
fn test_get_user_records() {
    let mut e = engine();
    e.settle(user(), provider(), 1_000, ServiceType::ModelInference, 1, 1_000).unwrap();
    e.settle(user(), provider2(), 2_000, ServiceType::ToolExecution, 2, 2_000).unwrap();
    // Different user — should not appear
    e.settle(provider(), provider2(), 500, ServiceType::Storage, 3, 3_000).unwrap();

    let recs = e.get_user_records(&user());
    assert_eq!(recs.len(), 2);
    for r in &recs {
        assert_eq!(r.user, user());
    }
}

#[test]
fn test_get_provider_records() {
    let mut e = engine();
    e.settle(user(), provider(), 1_000, ServiceType::ModelInference, 1, 1_000).unwrap();
    e.settle(user(), provider(), 2_000, ServiceType::ToolExecution, 2, 2_000).unwrap();
    e.settle(user(), provider2(), 3_000, ServiceType::Storage, 3, 3_000).unwrap();

    let recs = e.get_provider_records(&provider());
    assert_eq!(recs.len(), 2);
    for r in &recs {
        assert_eq!(r.provider, provider());
    }
}

// ---- Totals tracking tests -----------------------------------------

#[test]
fn test_total_tracking() {
    let mut e = engine();
    e.settle(user(), provider(), 1_000_000, ServiceType::ModelInference, 1, 1_000).unwrap();
    e.settle(user(), provider2(), 500_000, ServiceType::ToolExecution, 2, 2_000).unwrap();

    assert_eq!(e.get_total_settled(), 1_500_000);
    // fees: 25_000 + 12_500 = 37_500
    assert_eq!(e.get_total_fees(), 37_500);
}

// ---- Record fields test -------------------------------------------

#[test]
fn test_settlement_record_fields() {
    let mut e = engine();
    let rec = e
        .settle(user(), provider(), GROSS, ServiceType::AgentRuntime, 42, 99_000)
        .unwrap();

    assert_eq!(rec.height, 42);
    assert_eq!(rec.timestamp, 99_000);
    assert_eq!(rec.service_type, ServiceType::AgentRuntime);
    assert_eq!(rec.gross_amount, GROSS);
    assert_eq!(rec.fee_amount + rec.net_amount, GROSS);
    assert_ne!(rec.burn_tx, Hash::ZERO);
    assert_ne!(rec.mint_tx, Hash::ZERO);
    assert_ne!(rec.burn_tx, rec.mint_tx);
    assert_ne!(rec.id, Hash::ZERO);
}

// ---- Multiple service types test ----------------------------------

#[test]
fn test_multiple_service_types() {
    let mut e = engine();
    let services = vec![
        ServiceType::ModelInference,
        ServiceType::ToolExecution,
        ServiceType::ComputeUsage,
        ServiceType::Storage,
        ServiceType::AgentRuntime,
        ServiceType::Custom("NFTMinting".to_string()),
    ];

    for (i, svc) in services.into_iter().enumerate() {
        let user_addr = Address::from([(i as u8) * 2 + 1; 20]);
        let prov_addr = Address::from([(i as u8) * 2 + 2; 20]);
        let result = e.settle(user_addr, prov_addr, 10_000, svc, i as u64, i as u64 * 1_000);
        assert!(result.is_ok(), "settle failed for service index {}", i);
    }

    assert_eq!(e.records.len(), 6);
}
