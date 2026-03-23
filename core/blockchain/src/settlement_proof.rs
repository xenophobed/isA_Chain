use crate::settlement::ServiceType;
use crate::types::{Address, Amount, BlockHeight, Hash, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// ProofStatus
// ============================================================================

/// Lifecycle state of a settlement proof.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofStatus {
    /// Transaction submitted to chain, awaiting confirmations.
    Pending,
    /// Confirmed with enough block confirmations (>= confirmations_required).
    Confirmed,
    /// Deeply confirmed and considered immutable (>= finalization_depth).
    Finalized,
    /// Transaction failed; reason attached.
    Failed(String),
}

// ============================================================================
// SettlementProofRecord
// ============================================================================

/// On-chain proof that links an off-chain billing event to an on-chain
/// transaction hash, anchoring the billing record permanently.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettlementProofRecord {
    /// Unique proof ID (deterministic hash of inputs).
    pub proof_id: Hash,
    /// References the `SettlementRecord` this proof covers.
    pub settlement_id: Hash,
    /// On-chain transaction hash that executed the settlement.
    pub tx_hash: Hash,
    /// Off-chain billing event ID (e.g. from isA billing service).
    pub billing_event_id: String,
    /// Address that paid (ISA burned from this address).
    pub user: Address,
    /// Address that received payment (ISA minted to this address).
    pub provider: Address,
    /// Gross billed amount.
    pub amount: Amount,
    /// Protocol fee portion of the amount.
    pub fee: Amount,
    /// Which isA service was billed.
    pub service_type: ServiceType,
    /// Block height at which the transaction was included.
    pub block_height: BlockHeight,
    /// Unix timestamp (milliseconds) of proof creation.
    pub timestamp: Timestamp,
    /// Number of confirmations the transaction has accumulated.
    pub confirmations: u64,
    /// Current proof lifecycle status.
    pub status: ProofStatus,
}

// ============================================================================
// SettlementProofError
// ============================================================================

/// Errors that can arise during proof recording or verification.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SettlementProofError {
    #[error("Proof not found: {0:?}")]
    ProofNotFound(Hash),

    #[error("Duplicate proof for settlement: {0:?}")]
    DuplicateProof(Hash),

    #[error("Billing event already recorded: {0}")]
    DuplicateBillingEvent(String),

    #[error("Proof is invalid")]
    InvalidProof,

    #[error("Settlement not found: {0:?}")]
    SettlementNotFound(Hash),
}

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
            .ok_or_else(|| SettlementProofError::ProofNotFound(*proof_id))?;

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
            .ok_or_else(|| SettlementProofError::ProofNotFound(*proof_id))?;

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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Helpers -------------------------------------------------------

    fn user() -> Address {
        Address::from([0xAA; 20])
    }

    fn provider() -> Address {
        Address::from([0xBB; 20])
    }

    fn settlement_id() -> Hash {
        Hash::hash_data(b"settlement-1")
    }

    fn tx_hash() -> Hash {
        Hash::hash_data(b"tx-hash-1")
    }

    fn make_store() -> ProofStore {
        ProofStore::new(6, 100)
    }

    fn record_default(store: &mut ProofStore) -> Hash {
        store
            .record_proof(
                settlement_id(),
                tx_hash(),
                "billing-event-001".to_string(),
                user(),
                provider(),
                1_000_000,
                25_000,
                ServiceType::ModelInference,
                10,
                1_000_000,
            )
            .unwrap()
    }

    // ---- Core recording tests ------------------------------------------

    #[test]
    fn test_record_proof() {
        let mut store = make_store();
        let proof_id = record_default(&mut store);

        assert_ne!(proof_id, Hash::ZERO);
        let rec = store.get_proof(&proof_id).unwrap();
        assert_eq!(rec.settlement_id, settlement_id());
        assert_eq!(rec.tx_hash, tx_hash());
        assert_eq!(rec.billing_event_id, "billing-event-001");
        assert_eq!(rec.user, user());
        assert_eq!(rec.provider, provider());
        assert_eq!(rec.amount, 1_000_000);
        assert_eq!(rec.fee, 25_000);
        assert_eq!(rec.block_height, 10);
        assert_eq!(rec.confirmations, 0);
        assert_eq!(rec.status, ProofStatus::Pending);
    }

    #[test]
    fn test_duplicate_proof_fails() {
        let mut store = make_store();
        record_default(&mut store);

        // Same settlement_id should fail.
        let result = store.record_proof(
            settlement_id(),
            Hash::hash_data(b"tx-hash-2"),
            "billing-event-002".to_string(),
            user(),
            provider(),
            500_000,
            12_500,
            ServiceType::ToolExecution,
            20,
            2_000_000,
        );

        assert!(matches!(
            result,
            Err(SettlementProofError::DuplicateProof(_))
        ));
    }

    #[test]
    fn test_duplicate_billing_event() {
        let mut store = make_store();
        record_default(&mut store);

        // Different settlement but same billing_event_id should fail.
        let result = store.record_proof(
            Hash::hash_data(b"settlement-2"),
            Hash::hash_data(b"tx-hash-2"),
            "billing-event-001".to_string(), // duplicate
            user(),
            provider(),
            500_000,
            12_500,
            ServiceType::ToolExecution,
            20,
            2_000_000,
        );

        assert!(matches!(
            result,
            Err(SettlementProofError::DuplicateBillingEvent(_))
        ));
    }

    // ---- Confirmation lifecycle tests ----------------------------------

    #[test]
    fn test_update_confirmations_pending() {
        let mut store = make_store(); // needs 6 to confirm
        let proof_id = record_default(&mut store); // block_height = 10

        // current_height = 14 → confirmations = 4 (< 6) → still Pending
        let status = store.update_confirmations(&proof_id, 14).unwrap();
        assert_eq!(status, ProofStatus::Pending);

        let rec = store.get_proof(&proof_id).unwrap();
        assert_eq!(rec.confirmations, 4);
    }

    #[test]
    fn test_update_confirmations_confirmed() {
        let mut store = make_store(); // confirmations_required = 6
        let proof_id = record_default(&mut store); // block_height = 10

        // current_height = 16 → confirmations = 6 → Confirmed
        let status = store.update_confirmations(&proof_id, 16).unwrap();
        assert_eq!(status, ProofStatus::Confirmed);
    }

    #[test]
    fn test_update_confirmations_finalized() {
        let mut store = make_store(); // finalization_depth = 100
        let proof_id = record_default(&mut store); // block_height = 10

        // current_height = 110 → confirmations = 100 → Finalized
        let status = store.update_confirmations(&proof_id, 110).unwrap();
        assert_eq!(status, ProofStatus::Finalized);

        let rec = store.get_proof(&proof_id).unwrap();
        assert_eq!(rec.confirmations, 100);
    }

    #[test]
    fn test_update_confirmations_not_found() {
        let mut store = make_store();
        let unknown = Hash::hash_data(b"nonexistent");
        let result = store.update_confirmations(&unknown, 100);
        assert!(matches!(
            result,
            Err(SettlementProofError::ProofNotFound(_))
        ));
    }

    // ---- Lookup tests --------------------------------------------------

    #[test]
    fn test_get_by_settlement() {
        let mut store = make_store();
        let proof_id = record_default(&mut store);

        let rec = store.get_proof_by_settlement(&settlement_id()).unwrap();
        assert_eq!(rec.proof_id, proof_id);
    }

    #[test]
    fn test_get_by_billing_event() {
        let mut store = make_store();
        let proof_id = record_default(&mut store);

        let rec = store
            .get_proof_by_billing_event("billing-event-001")
            .unwrap();
        assert_eq!(rec.proof_id, proof_id);
    }

    #[test]
    fn test_get_user_proofs() {
        let mut store = make_store();

        // Two proofs for the same user, different settlements/billing events.
        store
            .record_proof(
                Hash::hash_data(b"s1"),
                Hash::hash_data(b"tx1"),
                "evt-1".to_string(),
                user(),
                provider(),
                1_000,
                25,
                ServiceType::ModelInference,
                1,
                1_000,
            )
            .unwrap();

        store
            .record_proof(
                Hash::hash_data(b"s2"),
                Hash::hash_data(b"tx2"),
                "evt-2".to_string(),
                user(),
                provider(),
                2_000,
                50,
                ServiceType::ToolExecution,
                2,
                2_000,
            )
            .unwrap();

        // Proof from a different user — should NOT appear.
        let other_user = Address::from([0xCC; 20]);
        store
            .record_proof(
                Hash::hash_data(b"s3"),
                Hash::hash_data(b"tx3"),
                "evt-3".to_string(),
                other_user,
                provider(),
                3_000,
                75,
                ServiceType::Storage,
                3,
                3_000,
            )
            .unwrap();

        let proofs = store.get_user_proofs(&user());
        assert_eq!(proofs.len(), 2);
        for p in &proofs {
            assert_eq!(p.user, user());
        }
    }

    // ---- Verification tests --------------------------------------------

    #[test]
    fn test_verify_proof() {
        let mut store = make_store();
        let proof_id = record_default(&mut store); // block_height = 10

        // Pending → not verified yet
        assert_eq!(store.verify_proof(&proof_id).unwrap(), false);

        // Advance to Confirmed
        store.update_confirmations(&proof_id, 16).unwrap();
        assert_eq!(store.verify_proof(&proof_id).unwrap(), true);

        // Advance to Finalized — still verified
        store.update_confirmations(&proof_id, 110).unwrap();
        assert_eq!(store.verify_proof(&proof_id).unwrap(), true);
    }

    #[test]
    fn test_verify_proof_not_found() {
        let store = make_store();
        let unknown = Hash::hash_data(b"ghost");
        let result = store.verify_proof(&unknown);
        assert!(matches!(
            result,
            Err(SettlementProofError::ProofNotFound(_))
        ));
    }

    #[test]
    fn test_verify_failed_proof() {
        let mut store = make_store();
        let proof_id = record_default(&mut store);

        // Manually set to Failed.
        store.proofs.get_mut(&proof_id).unwrap().status =
            ProofStatus::Failed("reverted".to_string());

        let result = store.verify_proof(&proof_id);
        assert!(matches!(result, Err(SettlementProofError::InvalidProof)));
    }

    // ---- Count tests ---------------------------------------------------

    #[test]
    fn test_pending_confirmed_counts() {
        let mut store = make_store(); // confirmations_required = 6

        // Record two proofs at different block heights.
        let p1 = store
            .record_proof(
                Hash::hash_data(b"s1"),
                Hash::hash_data(b"tx1"),
                "evt-1".to_string(),
                user(),
                provider(),
                1_000,
                25,
                ServiceType::ModelInference,
                10,
                1_000,
            )
            .unwrap();

        let p2 = store
            .record_proof(
                Hash::hash_data(b"s2"),
                Hash::hash_data(b"tx2"),
                "evt-2".to_string(),
                user(),
                provider(),
                2_000,
                50,
                ServiceType::ToolExecution,
                10,
                2_000,
            )
            .unwrap();

        assert_eq!(store.get_pending_count(), 2);
        assert_eq!(store.get_confirmed_count(), 0);

        // Confirm p1 only.
        store.update_confirmations(&p1, 16).unwrap();

        assert_eq!(store.get_pending_count(), 1);
        assert_eq!(store.get_confirmed_count(), 1);

        // Finalize p1 — confirmed count should drop.
        store.update_confirmations(&p1, 110).unwrap();

        assert_eq!(store.get_pending_count(), 1);
        assert_eq!(store.get_confirmed_count(), 0);

        // Confirm p2.
        store.update_confirmations(&p2, 16).unwrap();
        assert_eq!(store.get_confirmed_count(), 1);
    }
}
