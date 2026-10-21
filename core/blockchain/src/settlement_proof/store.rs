use super::types::{ProofStatus, SettlementProofError, SettlementProofRecord};
use crate::settlement::ServiceType;
use crate::types::{Address, Amount, BlockHeight, Hash, Timestamp};
use std::collections::HashMap;

// ============================================================================
// ProofStore
// ============================================================================

/// Storage and lifecycle manager for settlement proof records.
///
/// Maintains multiple indices for efficient lookup by proof ID,
/// settlement ID, billing event ID, user, and provider.
///
/// Status transitions:
///   `Pending` → `Confirmed` (once confirmations >= confirmations_required)
///   `Confirmed` → `Finalized` (once confirmations >= finalization_depth)
pub struct ProofStore {
    /// All proofs keyed by their proof_id.
    pub proofs: HashMap<Hash, SettlementProofRecord>,
    /// settlement_id → proof_id (one proof per settlement).
    pub by_settlement: HashMap<Hash, Hash>,
    /// billing_event_id → proof_id (one proof per billing event).
    pub by_billing_event: HashMap<String, Hash>,
    /// user address → list of proof_ids.
    pub by_user: HashMap<Address, Vec<Hash>>,
    /// provider address → list of proof_ids.
    pub by_provider: HashMap<Address, Vec<Hash>>,
    /// Minimum confirmations required to transition to `Confirmed`.
    pub confirmations_required: u64,
    /// Confirmations required to transition to `Finalized`.
    pub finalization_depth: u64,
}

impl ProofStore {
    // ----------------------------------------------------------------
    // Constructor
    // ----------------------------------------------------------------

    /// Create a new store with explicit confirmation thresholds.
    ///
    /// Typical production values: `confirmations_required = 6`,
    /// `finalization_depth = 100`.
    pub fn new(confirmations_required: u64, finalization_depth: u64) -> Self {
        ProofStore {
            proofs: HashMap::new(),
            by_settlement: HashMap::new(),
            by_billing_event: HashMap::new(),
            by_user: HashMap::new(),
            by_provider: HashMap::new(),
            confirmations_required,
            finalization_depth,
        }
    }

    // ----------------------------------------------------------------
    // Recording
    // ----------------------------------------------------------------

    /// Record a new settlement proof.
    ///
    /// Returns the generated `proof_id` on success.
    ///
    /// Fails with:
    /// - `DuplicateProof` if a proof for `settlement_id` already exists.
    /// - `DuplicateBillingEvent` if `billing_event_id` was already recorded.
    #[allow(clippy::too_many_arguments)]
    pub fn record_proof(
        &mut self,
        settlement_id: Hash,
        tx_hash: Hash,
        billing_event_id: String,
        user: Address,
        provider: Address,
        amount: Amount,
        fee: Amount,
        service_type: ServiceType,
        height: BlockHeight,
        timestamp: Timestamp,
    ) -> Result<Hash, SettlementProofError> {
        // --- Duplicate checks ------------------------------------------
        if self.by_settlement.contains_key(&settlement_id) {
            return Err(SettlementProofError::DuplicateProof(settlement_id));
        }
        if self.by_billing_event.contains_key(&billing_event_id) {
            return Err(SettlementProofError::DuplicateBillingEvent(
                billing_event_id,
            ));
        }

        // --- Deterministic proof_id ------------------------------------
        let mut id_input = Vec::new();
        id_input.extend_from_slice(settlement_id.as_bytes());
        id_input.extend_from_slice(tx_hash.as_bytes());
        id_input.extend_from_slice(billing_event_id.as_bytes());
        id_input.extend_from_slice(user.as_bytes());
        id_input.extend_from_slice(provider.as_bytes());
        id_input.extend_from_slice(&amount.to_le_bytes());
        id_input.extend_from_slice(&height.to_le_bytes());
        id_input.extend_from_slice(&timestamp.to_le_bytes());
        let proof_id = Hash::hash_data(&id_input);

        // --- Build record ----------------------------------------------
        let record = SettlementProofRecord {
            proof_id,
            settlement_id,
            tx_hash,
            billing_event_id: billing_event_id.clone(),
            user,
            provider,
            amount,
            fee,
            service_type,
            block_height: height,
            timestamp,
            confirmations: 0,
            status: ProofStatus::Pending,
        };

        // --- Insert into indices ---------------------------------------
        self.by_settlement.insert(settlement_id, proof_id);
        self.by_billing_event.insert(billing_event_id, proof_id);
        self.by_user.entry(user).or_default().push(proof_id);
        self.by_provider.entry(provider).or_default().push(proof_id);
        self.proofs.insert(proof_id, record);

        Ok(proof_id)
    }

    // ----------------------------------------------------------------
    // Confirmation lifecycle
    // ----------------------------------------------------------------

    /// Update confirmation count and advance status accordingly.
    ///
    /// `current_height` is the latest known block height.  Confirmations
    /// are computed as `current_height - block_height`.
    ///
    /// Returns the new `ProofStatus` on success.
    pub fn update_confirmations(
        &mut self,
        proof_id: &Hash,
        current_height: BlockHeight,
    ) -> Result<ProofStatus, SettlementProofError> {
        let record = self
            .proofs
            .get_mut(proof_id)
            .ok_or(SettlementProofError::ProofNotFound(*proof_id))?;

        // Do not regress a Failed proof.
        if let ProofStatus::Failed(_) = &record.status {
            return Ok(record.status.clone());
        }

        let confirmations = current_height.saturating_sub(record.block_height);
        record.confirmations = confirmations;

        record.status = if confirmations >= self.finalization_depth {
            ProofStatus::Finalized
        } else if confirmations >= self.confirmations_required {
            ProofStatus::Confirmed
        } else {
            ProofStatus::Pending
        };

        Ok(record.status.clone())
    }

    // ----------------------------------------------------------------
    // Lookups
    // ----------------------------------------------------------------

    /// Retrieve a proof by its unique `proof_id`.
    pub fn get_proof(&self, proof_id: &Hash) -> Option<&SettlementProofRecord> {
        self.proofs.get(proof_id)
    }

    /// Retrieve the proof linked to a specific `settlement_id`.
    pub fn get_proof_by_settlement(
        &self,
        settlement_id: &Hash,
    ) -> Option<&SettlementProofRecord> {
        let proof_id = self.by_settlement.get(settlement_id)?;
        self.proofs.get(proof_id)
    }

    /// Retrieve the proof linked to a specific `billing_event_id`.
    pub fn get_proof_by_billing_event(
        &self,
        billing_event_id: &str,
    ) -> Option<&SettlementProofRecord> {
        let proof_id = self.by_billing_event.get(billing_event_id)?;
        self.proofs.get(proof_id)
    }

    /// All proofs where `user` is the payer.
    pub fn get_user_proofs(&self, user: &Address) -> Vec<&SettlementProofRecord> {
        self.by_user
            .get(user)
            .map(|ids| ids.iter().filter_map(|id| self.proofs.get(id)).collect())
            .unwrap_or_default()
    }

    /// All proofs where `provider` is the recipient.
    pub fn get_provider_proofs(&self, provider: &Address) -> Vec<&SettlementProofRecord> {
        self.by_provider
            .get(provider)
            .map(|ids| ids.iter().filter_map(|id| self.proofs.get(id)).collect())
            .unwrap_or_default()
    }

    // ----------------------------------------------------------------
    // Verification
    // ----------------------------------------------------------------

    /// Verify that a proof exists and is at least `Confirmed`.
    ///
    /// Returns `Ok(true)` for Confirmed/Finalized, `Ok(false)` for Pending,
    /// and `Err(InvalidProof)` for Failed proofs.
    pub fn verify_proof(&self, proof_id: &Hash) -> Result<bool, SettlementProofError> {
        let record = self
            .proofs
            .get(proof_id)
            .ok_or(SettlementProofError::ProofNotFound(*proof_id))?;

        match &record.status {
            ProofStatus::Confirmed | ProofStatus::Finalized => Ok(true),
            ProofStatus::Pending => Ok(false),
            ProofStatus::Failed(_) => Err(SettlementProofError::InvalidProof),
        }
    }

    // ----------------------------------------------------------------
    // Aggregate counts
    // ----------------------------------------------------------------

    /// Number of proofs in `Pending` status.
    pub fn get_pending_count(&self) -> usize {
        self.proofs
            .values()
            .filter(|r| r.status == ProofStatus::Pending)
            .count()
    }

    /// Number of proofs in `Confirmed` status (excludes Finalized).
    pub fn get_confirmed_count(&self) -> usize {
        self.proofs
            .values()
            .filter(|r| r.status == ProofStatus::Confirmed)
            .count()
    }
}
