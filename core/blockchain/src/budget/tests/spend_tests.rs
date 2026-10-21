use super::*;
use crate::budget::types::{BudgetError, DelegationStatus};
use crate::settlement::ServiceType;

#[test]
fn test_spend_success() {
    let mut mgr = make_manager();
    let id = create_simple(&mut mgr);

    mgr.spend(&id, ONE_ISA / 2, &ServiceType::ModelInference, 1)
        .expect("spend should succeed");

    let d = mgr.get_delegation(&id).unwrap();
    assert_eq!(d.spent, ONE_ISA / 2);
    assert_eq!(d.status, DelegationStatus::Active);
}

#[test]
fn test_spend_exceeds_per_tx() {
    let mut mgr = make_manager();
    let id = create_simple(&mut mgr); // per_tx_limit = ONE_ISA

    let result = mgr.spend(&id, ONE_ISA + 1, &ServiceType::ModelInference, 1);
    assert!(matches!(result, Err(BudgetError::ExceedsPerTxLimit { .. })));
}

#[test]
fn test_spend_exceeds_budget() {
    let mut mgr = make_manager();
    // total=2, per_tx=2, so two per-tx limit spends would exhaust budget
    let id = mgr
        .create_delegation(delegator(), agent(), 2, 2, None, 1_000, 0)
        .unwrap();

    // First spend succeeds (2 of 2 used)
    mgr.spend(&id, 2, &ServiceType::ToolExecution, 1).unwrap();

    // Second spend should fail with ExceedsBudget (remaining=0)
    let result = mgr.spend(&id, 1, &ServiceType::ToolExecution, 2);
    // After exhaustion the status flips, so we might see BudgetExhausted
    assert!(matches!(
        result,
        Err(BudgetError::BudgetExhausted) | Err(BudgetError::ExceedsBudget { .. })
    ));
}

#[test]
fn test_spend_expired() {
    let mut mgr = make_manager();
    let id = create_simple(&mut mgr); // expires_at = 1_000

    // Attempt to spend past expiry
    let result = mgr.spend(&id, ONE_ISA / 2, &ServiceType::ModelInference, 1_001);
    assert_eq!(result, Err(BudgetError::DelegationExpired));
}

#[test]
fn test_spend_service_not_allowed() {
    let mut mgr = make_manager();
    let id = mgr
        .create_delegation(
            delegator(),
            agent(),
            ONE_ISA * 10,
            ONE_ISA,
            Some(vec![ServiceType::ModelInference]),
            1_000,
            0,
        )
        .unwrap();

    let result = mgr.spend(&id, 100, &ServiceType::ToolExecution, 1);
    assert_eq!(result, Err(BudgetError::ServiceNotAllowed));
}

#[test]
fn test_get_available_budget() {
    let mut mgr = make_manager();
    let id = create_simple(&mut mgr); // total=10 ISA, expires_at=1_000

    assert_eq!(
        mgr.get_available_budget(&id, 0).unwrap(),
        ONE_ISA * 10
    );

    mgr.spend(&id, ONE_ISA, &ServiceType::AgentRuntime, 1).unwrap();
    assert_eq!(
        mgr.get_available_budget(&id, 1).unwrap(),
        ONE_ISA * 9
    );

    // Past expiry → returns 0
    assert_eq!(mgr.get_available_budget(&id, 1_001).unwrap(), 0);
}

#[test]
fn test_check_expired() {
    let mut mgr = make_manager();
    let id = create_simple(&mut mgr); // expires_at=1_000

    // Not yet expired
    assert!(!mgr.check_expired(&id, 999));
    assert_eq!(
        mgr.get_delegation(&id).unwrap().status,
        DelegationStatus::Active
    );

    // Exactly at expiry block (expires_at=1000, current=1000 — NOT expired, > not >=)
    assert!(!mgr.check_expired(&id, 1_000));

    // One block past expiry
    assert!(mgr.check_expired(&id, 1_001));
    assert_eq!(
        mgr.get_delegation(&id).unwrap().status,
        DelegationStatus::Expired
    );
}

#[test]
fn test_delegation_not_found() {
    let mut mgr = make_manager();
    let missing_id = Hash::new([0xFF; 32]);

    assert!(matches!(
        mgr.spend(&missing_id, 1, &ServiceType::ModelInference, 0),
        Err(BudgetError::DelegationNotFound(_))
    ));
    assert!(matches!(
        mgr.pause(&missing_id, &delegator()),
        Err(BudgetError::DelegationNotFound(_))
    ));
}
