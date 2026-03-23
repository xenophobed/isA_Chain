use crate::settlement::ServiceType;
use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// DelegationStatus
// ============================================================================

/// Lifecycle state of a budget delegation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelegationStatus {
    /// Delegation is active; agent may spend.
    Active,
    /// Temporarily paused by the delegator; spending blocked.
    Paused,
    /// Total budget has been fully consumed.
    Exhausted,
    /// Delegation passed its expiry block height.
    Expired,
    /// Explicitly cancelled by the delegator.
    Revoked,
}

// ============================================================================
// BudgetDelegation
// ============================================================================

/// A record authorising an agent to spend on behalf of a human delegator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BudgetDelegation {
    /// Unique identifier for this delegation.
    pub id: Hash,
    /// Human address that created the delegation.
    pub delegator: Address,
    /// Agent wallet that may draw from this budget.
    pub agent: Address,
    /// Total credits authorised by the delegator.
    pub total_budget: Amount,
    /// Amount already spent against this delegation.
    pub spent: Amount,
    /// Maximum credits that can be spent in a single transaction.
    pub per_tx_limit: Amount,
    /// Restrict spending to specific service types; `None` means all services.
    pub allowed_services: Option<Vec<ServiceType>>,
    /// Block height after which the delegation is no longer valid.
    pub expires_at: BlockHeight,
    /// Current lifecycle status.
    pub status: DelegationStatus,
    /// Block height at which the delegation was created.
    pub created_at: BlockHeight,
}

// ============================================================================
// BudgetError
// ============================================================================

/// Errors that can arise from budget delegation operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BudgetError {
    #[error("Delegation not found: {0}")]
    DelegationNotFound(Hash),

    #[error("Budget exhausted")]
    BudgetExhausted,

    #[error("Exceeds per-transaction limit: limit {limit}, requested {requested}")]
    ExceedsPerTxLimit { limit: Amount, requested: Amount },

    #[error("Exceeds remaining budget: remaining {remaining}, requested {requested}")]
    ExceedsBudget { remaining: Amount, requested: Amount },

    #[error("Delegation has expired")]
    DelegationExpired,

    #[error("Delegation is not active")]
    DelegationNotActive,

    #[error("Unauthorized delegator: {0}")]
    UnauthorizedDelegator(Address),

    #[error("Service type not allowed by this delegation")]
    ServiceNotAllowed,

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount,

    #[error("A delegation between this delegator and agent already exists")]
    DuplicateDelegation,
}

// ============================================================================
// BudgetManager
// ============================================================================

/// Manages all budget delegations on-chain.
pub struct BudgetManager {
    /// All delegations keyed by their ID.
    pub delegations: HashMap<Hash, BudgetDelegation>,
    /// Index: delegator address → list of delegation IDs.
    pub by_delegator: HashMap<Address, Vec<Hash>>,
    /// Index: agent address → list of delegation IDs.
    pub by_agent: HashMap<Address, Vec<Hash>>,
}

impl BudgetManager {
    /// Create a new, empty `BudgetManager`.
    pub fn new() -> Self {
        BudgetManager {
            delegations: HashMap::new(),
            by_delegator: HashMap::new(),
            by_agent: HashMap::new(),
        }
    }

    /// Create a new budget delegation.
    ///
    /// Returns the delegation ID (`Hash`) on success.
    pub fn create_delegation(
        &mut self,
        delegator: Address,
        agent: Address,
        total_budget: Amount,
        per_tx_limit: Amount,
        allowed_services: Option<Vec<ServiceType>>,
        expires_at: BlockHeight,
        height: BlockHeight,
    ) -> Result<Hash, BudgetError> {
        if total_budget == 0 || per_tx_limit == 0 {
            return Err(BudgetError::InvalidAmount);
        }

        // Derive a deterministic ID from delegator + agent + height.
        let mut id_input = Vec::with_capacity(20 + 20 + 8);
        id_input.extend_from_slice(delegator.as_bytes());
        id_input.extend_from_slice(agent.as_bytes());
        id_input.extend_from_slice(&height.to_le_bytes());
        let id = Hash::hash_data(&id_input);

        if self.delegations.contains_key(&id) {
            return Err(BudgetError::DuplicateDelegation);
        }

        let delegation = BudgetDelegation {
            id,
            delegator,
            agent,
            total_budget,
            spent: 0,
            per_tx_limit,
            allowed_services,
            expires_at,
            status: DelegationStatus::Active,
            created_at: height,
        };

        self.delegations.insert(id, delegation);
        self.by_delegator.entry(delegator).or_default().push(id);
        self.by_agent.entry(agent).or_default().push(id);

        Ok(id)
    }

    /// Deduct `amount` from a delegation's budget.
    ///
    /// Validates all constraints (status, expiry, per-tx limit, remaining
    /// budget, service allowlist) before mutating state.
    pub fn spend(
        &mut self,
        delegation_id: &Hash,
        amount: Amount,
        service_type: &ServiceType,
        current_height: BlockHeight,
    ) -> Result<(), BudgetError> {
        if amount == 0 {
            return Err(BudgetError::InvalidAmount);
        }

        // Check expiry first (and mark if necessary) before borrow.
        let _ = self.check_expired(delegation_id, current_height);

        let delegation = self
            .delegations
            .get_mut(delegation_id)
            .ok_or_else(|| BudgetError::DelegationNotFound(*delegation_id))?;

        // Status checks
        match &delegation.status {
            DelegationStatus::Expired => return Err(BudgetError::DelegationExpired),
            DelegationStatus::Exhausted => return Err(BudgetError::BudgetExhausted),
            DelegationStatus::Revoked | DelegationStatus::Paused => {
                return Err(BudgetError::DelegationNotActive)
            }
            DelegationStatus::Active => {}
        }

        // Per-transaction limit
        if amount > delegation.per_tx_limit {
            return Err(BudgetError::ExceedsPerTxLimit {
                limit: delegation.per_tx_limit,
                requested: amount,
            });
        }

        // Remaining budget
        let remaining = delegation.total_budget.saturating_sub(delegation.spent);
        if amount > remaining {
            return Err(BudgetError::ExceedsBudget { remaining, requested: amount });
        }

        // Service allowlist
        if let Some(ref allowed) = delegation.allowed_services {
            if !allowed.contains(service_type) {
                return Err(BudgetError::ServiceNotAllowed);
            }
        }

        // Apply spend
        delegation.spent += amount;
        if delegation.spent >= delegation.total_budget {
            delegation.status = DelegationStatus::Exhausted;
        }

        Ok(())
    }

    /// Look up a delegation by ID.
    pub fn get_delegation(&self, id: &Hash) -> Option<&BudgetDelegation> {
        self.delegations.get(id)
    }

    /// Return all delegations created by a given delegator.
    pub fn get_delegator_budgets(&self, delegator: &Address) -> Vec<&BudgetDelegation> {
        match self.by_delegator.get(delegator) {
            Some(ids) => ids.iter().filter_map(|id| self.delegations.get(id)).collect(),
            None => Vec::new(),
        }
    }

    /// Return all delegations assigned to a given agent.
    pub fn get_agent_budgets(&self, agent: &Address) -> Vec<&BudgetDelegation> {
        match self.by_agent.get(agent) {
            Some(ids) => ids.iter().filter_map(|id| self.delegations.get(id)).collect(),
            None => Vec::new(),
        }
    }

    /// Return the remaining budget for a delegation.
    ///
    /// Returns `0` if the delegation is expired; otherwise `total - spent`.
    pub fn get_available_budget(
        &self,
        id: &Hash,
        current_height: BlockHeight,
    ) -> Result<Amount, BudgetError> {
        let delegation = self
            .delegations
            .get(id)
            .ok_or_else(|| BudgetError::DelegationNotFound(*id))?;

        if current_height > delegation.expires_at {
            return Ok(0);
        }

        Ok(delegation.total_budget.saturating_sub(delegation.spent))
    }

    /// Pause an active delegation (delegator-only).
    pub fn pause(&mut self, id: &Hash, delegator: &Address) -> Result<(), BudgetError> {
        let delegation = self
            .delegations
            .get_mut(id)
            .ok_or_else(|| BudgetError::DelegationNotFound(*id))?;

        if &delegation.delegator != delegator {
            return Err(BudgetError::UnauthorizedDelegator(*delegator));
        }
        if delegation.status != DelegationStatus::Active {
            return Err(BudgetError::DelegationNotActive);
        }

        delegation.status = DelegationStatus::Paused;
        Ok(())
    }

    /// Resume a paused delegation (delegator-only).
    pub fn resume(&mut self, id: &Hash, delegator: &Address) -> Result<(), BudgetError> {
        let delegation = self
            .delegations
            .get_mut(id)
            .ok_or_else(|| BudgetError::DelegationNotFound(*id))?;

        if &delegation.delegator != delegator {
            return Err(BudgetError::UnauthorizedDelegator(*delegator));
        }
        if delegation.status != DelegationStatus::Paused {
            return Err(BudgetError::DelegationNotActive);
        }

        delegation.status = DelegationStatus::Active;
        Ok(())
    }

    /// Revoke a delegation entirely (delegator-only).
    pub fn revoke(&mut self, id: &Hash, delegator: &Address) -> Result<(), BudgetError> {
        let delegation = self
            .delegations
            .get_mut(id)
            .ok_or_else(|| BudgetError::DelegationNotFound(*id))?;

        if &delegation.delegator != delegator {
            return Err(BudgetError::UnauthorizedDelegator(*delegator));
        }
        if matches!(
            delegation.status,
            DelegationStatus::Revoked | DelegationStatus::Expired
        ) {
            return Err(BudgetError::DelegationNotActive);
        }

        delegation.status = DelegationStatus::Revoked;
        Ok(())
    }

    /// Increase the total budget of an existing delegation (delegator-only).
    pub fn increase_budget(
        &mut self,
        id: &Hash,
        additional: Amount,
        delegator: &Address,
    ) -> Result<(), BudgetError> {
        if additional == 0 {
            return Err(BudgetError::InvalidAmount);
        }

        let delegation = self
            .delegations
            .get_mut(id)
            .ok_or_else(|| BudgetError::DelegationNotFound(*id))?;

        if &delegation.delegator != delegator {
            return Err(BudgetError::UnauthorizedDelegator(*delegator));
        }
        if matches!(
            delegation.status,
            DelegationStatus::Revoked | DelegationStatus::Expired
        ) {
            return Err(BudgetError::DelegationNotActive);
        }

        delegation.total_budget = delegation.total_budget.saturating_add(additional);

        // If it was Exhausted but now has headroom, re-activate.
        if delegation.status == DelegationStatus::Exhausted
            && delegation.total_budget > delegation.spent
        {
            delegation.status = DelegationStatus::Active;
        }

        Ok(())
    }

    /// Check whether a delegation has expired at `current_height`.
    ///
    /// If so, marks its status as `Expired` and returns `true`.
    /// Returns `false` if the delegation is not found or not yet expired.
    pub fn check_expired(&mut self, id: &Hash, current_height: BlockHeight) -> bool {
        if let Some(delegation) = self.delegations.get_mut(id) {
            if current_height > delegation.expires_at
                && delegation.status == DelegationStatus::Active
            {
                delegation.status = DelegationStatus::Expired;
                return true;
            }
        }
        false
    }
}

impl Default for BudgetManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement::ServiceType;

    const ONE_ISA: Amount = 1_000_000_000_000_000_000;

    fn delegator() -> Address {
        Address::from([0xAAu8; 20])
    }

    fn agent() -> Address {
        Address::from([0xBBu8; 20])
    }

    fn intruder() -> Address {
        Address::from([0xCCu8; 20])
    }

    fn make_manager() -> BudgetManager {
        BudgetManager::new()
    }

    /// Helper: create a simple delegation and return its ID.
    fn create_simple(mgr: &mut BudgetManager) -> Hash {
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

    // -----------------------------------------------------------------------
    // test_create_delegation
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // test_spend_success
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // test_spend_exceeds_per_tx
    // -----------------------------------------------------------------------

    #[test]
    fn test_spend_exceeds_per_tx() {
        let mut mgr = make_manager();
        let id = create_simple(&mut mgr); // per_tx_limit = ONE_ISA

        let result = mgr.spend(&id, ONE_ISA + 1, &ServiceType::ModelInference, 1);
        assert!(matches!(result, Err(BudgetError::ExceedsPerTxLimit { .. })));
    }

    // -----------------------------------------------------------------------
    // test_spend_exceeds_budget
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // test_spend_expired
    // -----------------------------------------------------------------------

    #[test]
    fn test_spend_expired() {
        let mut mgr = make_manager();
        let id = create_simple(&mut mgr); // expires_at = 1_000

        // Attempt to spend past expiry
        let result = mgr.spend(&id, ONE_ISA / 2, &ServiceType::ModelInference, 1_001);
        assert_eq!(result, Err(BudgetError::DelegationExpired));
    }

    // -----------------------------------------------------------------------
    // test_spend_service_not_allowed
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // test_pause_resume
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // test_revoke
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // test_increase_budget
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // test_get_available_budget
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // test_check_expired
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // test_unauthorized_delegator
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Additional: delegation_not_found
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Additional: invalid amount on create
    // -----------------------------------------------------------------------

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
}
