use crate::budget::manager::BudgetManager;
use crate::types::{Address, Amount, Hash};

mod delegation_tests;
mod spend_tests;

pub(crate) const ONE_ISA: Amount = 1_000_000_000_000_000_000;

pub(crate) fn delegator() -> Address {
    Address::from([0xAAu8; 20])
}

pub(crate) fn agent() -> Address {
    Address::from([0xBBu8; 20])
}

pub(crate) fn intruder() -> Address {
    Address::from([0xCCu8; 20])
}

pub(crate) fn make_manager() -> BudgetManager {
    BudgetManager::new()
}

/// Helper: create a simple delegation and return its ID.
pub(crate) fn create_simple(mgr: &mut BudgetManager) -> Hash {
    mgr.create_delegation(
        delegator(),
        agent(),
        ONE_ISA * 10,       // total_budget
        ONE_ISA,            // per_tx_limit
        None,               // all services
        1_000,              // expires_at
        0,                  // created at height 0
    )
    .expect("delegation should be created")
}
