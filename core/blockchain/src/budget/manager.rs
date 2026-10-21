use super::types::{BudgetDelegation, BudgetError, DelegationStatus};
use crate::settlement::ServiceType;
use crate::types::{Address, Amount, BlockHeight, Hash};
use std::collections::HashMap;

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
    #[allow(clippy::too_many_arguments)]
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
            .ok_or(BudgetError::DelegationNotFound(*delegation_id))?;

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
            .ok_or(BudgetError::DelegationNotFound(*id))?;

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
            .ok_or(BudgetError::DelegationNotFound(*id))?;

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
            .ok_or(BudgetError::DelegationNotFound(*id))?;

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
            .ok_or(BudgetError::DelegationNotFound(*id))?;

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
            .ok_or(BudgetError::DelegationNotFound(*id))?;

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
