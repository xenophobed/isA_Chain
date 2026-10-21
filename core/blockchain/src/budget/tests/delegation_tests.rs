use super::*;
use crate::budget::types::{BudgetError, DelegationStatus};
use crate::settlement::ServiceType;

#[test]
fn test_create_delegation() {
    let mut mgr = make_manager();
    let id = create_simple(&mut mgr);

    let d = mgr.get_delegation(&id).expect("delegation must exist");
    assert_eq!(d.delegator, delegator());
    assert_eq!(d.agent, agent());
    assert_eq!(d.total_budget, ONE_ISA * 10);
    assert_eq!(d.spent, 0);
    assert_eq!(d.per_tx_limit, ONE_ISA);
    assert_eq!(d.status, DelegationStatus::Active);
    assert_eq!(d.created_at, 0);
    assert_eq!(d.expires_at, 1_000);

    // Indexed correctly
    assert_eq!(mgr.get_delegator_budgets(&delegator()).len(), 1);
    assert_eq!(mgr.get_agent_budgets(&agent()).len(), 1);
}

#[test]
fn test_pause_resume() {
    let mut mgr = make_manager();
    let id = create_simple(&mut mgr);

    // Pause
    mgr.pause(&id, &delegator()).unwrap();
    assert_eq!(
        mgr.get_delegation(&id).unwrap().status,
        DelegationStatus::Paused
    );

    // Spending while paused fails
    let result = mgr.spend(&id, 100, &ServiceType::ModelInference, 1);
    assert_eq!(result, Err(BudgetError::DelegationNotActive));

    // Resume
    mgr.resume(&id, &delegator()).unwrap();
    assert_eq!(
        mgr.get_delegation(&id).unwrap().status,
        DelegationStatus::Active
    );

    // Spending now works
    mgr.spend(&id, 100, &ServiceType::ModelInference, 1).unwrap();
}

#[test]
fn test_revoke() {
    let mut mgr = make_manager();
    let id = create_simple(&mut mgr);

    mgr.revoke(&id, &delegator()).unwrap();
    assert_eq!(
        mgr.get_delegation(&id).unwrap().status,
        DelegationStatus::Revoked
    );

    // Spending after revocation fails
    let result = mgr.spend(&id, 100, &ServiceType::ModelInference, 1);
    assert_eq!(result, Err(BudgetError::DelegationNotActive));

    // Revoking again also fails
    let result = mgr.revoke(&id, &delegator());
    assert_eq!(result, Err(BudgetError::DelegationNotActive));
}

#[test]
fn test_increase_budget() {
    let mut mgr = make_manager();
    // total=100, per_tx=100
    let id = mgr
        .create_delegation(delegator(), agent(), 100, 100, None, 1_000, 0)
        .unwrap();

    // Exhaust the budget
    mgr.spend(&id, 100, &ServiceType::Storage, 1).unwrap();
    assert_eq!(
        mgr.get_delegation(&id).unwrap().status,
        DelegationStatus::Exhausted
    );

    // Increase budget → re-activates
    mgr.increase_budget(&id, 200, &delegator()).unwrap();
    let d = mgr.get_delegation(&id).unwrap();
    assert_eq!(d.total_budget, 300);
    assert_eq!(d.status, DelegationStatus::Active);

    // Can spend again
    mgr.spend(&id, 50, &ServiceType::Storage, 2).unwrap();
}

#[test]
fn test_unauthorized_delegator() {
    let mut mgr = make_manager();
    let id = create_simple(&mut mgr);

    assert_eq!(
        mgr.pause(&id, &intruder()),
        Err(BudgetError::UnauthorizedDelegator(intruder()))
    );
    assert_eq!(
        mgr.revoke(&id, &intruder()),
        Err(BudgetError::UnauthorizedDelegator(intruder()))
    );
    assert_eq!(
        mgr.increase_budget(&id, ONE_ISA, &intruder()),
        Err(BudgetError::UnauthorizedDelegator(intruder()))
    );
}

#[test]
fn test_invalid_amount_on_create() {
    let mut mgr = make_manager();
    assert_eq!(
        mgr.create_delegation(delegator(), agent(), 0, 100, None, 1_000, 0),
        Err(BudgetError::InvalidAmount)
    );
    assert_eq!(
        mgr.create_delegation(delegator(), agent(), 100, 0, None, 1_000, 0),
        Err(BudgetError::InvalidAmount)
    );
}
