use crate::types::{Address, Amount, BlockHeight, Hash, Timestamp};
use crate::types::constants::PROTOCOL_FEE_PERCENT;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// ServiceType
// ============================================================================

/// Which isA service was billed for this settlement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceType {
    /// isA_Model — model inference
    ModelInference,
    /// isA_MCP — tool execution
    ToolExecution,
    /// isA_OS — compute usage
    ComputeUsage,
    /// isA_Data — storage
    Storage,
    /// isA_Agent — agent runtime
    AgentRuntime,
    /// Extensible catch-all
    Custom(String),
}

// ============================================================================
// SettlementStatus
// ============================================================================

/// Lifecycle state of a settlement record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettlementStatus {
    Pending,
    Completed,
    /// Settlement failed; reason attached.
    Failed(String),
    Disputed,
}

// ============================================================================
// SettlementRecord
// ============================================================================

/// Immutable record of a single burn-mint settlement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettlementRecord {
    /// Unique settlement ID (hash of inputs).
    pub id: Hash,
    /// Address that paid (ISA burned from this address).
    pub user: Address,
    /// Address that received payment (ISA minted to this address).
    pub provider: Address,
    /// Total billed amount before fee deduction.
    pub gross_amount: Amount,
    /// Protocol fee deducted from gross.
    pub fee_amount: Amount,
    /// Net amount minted to provider (gross - fee).
    pub net_amount: Amount,
    /// Hash reference for the burn transaction.
    pub burn_tx: Hash,
    /// Hash reference for the mint transaction.
    pub mint_tx: Hash,
    /// Block height at which settlement was processed.
    pub height: BlockHeight,
    /// Unix timestamp (milliseconds) of settlement.
    pub timestamp: Timestamp,
    /// Which service was billed.
    pub service_type: ServiceType,
    /// Current lifecycle status.
    pub status: SettlementStatus,
}

// ============================================================================
// SettlementError
// ============================================================================

/// Errors that can arise during settlement processing.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SettlementError {
    #[error("Insufficient balance to settle")]
    InsufficientBalance,

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount,

    #[error("User and provider addresses must differ")]
    SameUserAndProvider,

    #[error("Settlement not found: {0:?}")]
    SettlementNotFound(Hash),

    #[error("Settlement already finalized: {0:?}")]
    AlreadySettled(Hash),

    #[error("Token error: {0}")]
    TokenError(String),
}

// ============================================================================
// SettlementEngine
// ============================================================================

/// On-chain settlement engine that processes burn-mint cycles.
///
/// For each settlement:
/// 1. Calculates protocol fee and net provider payment.
/// 2. Records a `SettlementRecord` with burn/mint transaction hashes.
/// 3. Maintains indices for fast user/provider lookups.
///
/// **Note:** This engine records accounting only.  Actual token-ledger
/// mutations (balance debit/credit) are the responsibility of the
/// higher-level `Blockchain` layer that calls this engine.
pub struct SettlementEngine {
    /// Ordered settlement history.
    pub records: Vec<SettlementRecord>,
    /// Index: user address → indices into `records`.
    pub records_by_user: HashMap<Address, Vec<usize>>,
    /// Index: provider address → indices into `records`.
    pub records_by_provider: HashMap<Address, Vec<usize>>,
    /// Running total of all gross amounts settled.
    pub total_settled: Amount,
    /// Running total of all protocol fees collected.
    pub total_fees: Amount,
    /// Protocol fee in basis points (e.g. 250 = 2.5%).
    pub fee_rate_bps: u32,
}

impl SettlementEngine {
    // ----------------------------------------------------------------
    // Constructor
    // ----------------------------------------------------------------

    /// Create a new engine with the given fee rate.
    ///
    /// Use `PROTOCOL_FEE_PERCENT` (250 bps = 2.5%) as the default.
    pub fn new(fee_rate_bps: u32) -> Self {
        SettlementEngine {
            records: Vec::new(),
            records_by_user: HashMap::new(),
            records_by_provider: HashMap::new(),
            total_settled: 0,
            total_fees: 0,
            fee_rate_bps,
        }
    }

    // ----------------------------------------------------------------
    // Fee math
    // ----------------------------------------------------------------

    /// Split `gross_amount` into `(fee, net)` using the engine fee rate.
    ///
    /// Uses integer arithmetic rounded down (floor).  The net amount is
    /// always `gross - fee`, never negative.
    pub fn calculate_split(&self, gross_amount: Amount) -> (Amount, Amount) {
        // fee = gross * bps / 10_000
        let fee = gross_amount
            .saturating_mul(self.fee_rate_bps as u128)
            / 10_000;
        let net = gross_amount.saturating_sub(fee);
        (fee, net)
    }

    // ----------------------------------------------------------------
    // Core settlement
    // ----------------------------------------------------------------

    /// Process a single settlement.
    ///
    /// Generates deterministic transaction hashes from the input data,
    /// records the settlement, updates the indices and running totals,
    /// and returns the completed `SettlementRecord`.
    pub fn settle(
        &mut self,
        user: Address,
        provider: Address,
        gross_amount: Amount,
        service_type: ServiceType,
        height: BlockHeight,
        timestamp: Timestamp,
    ) -> Result<SettlementRecord, SettlementError> {
        // --- Validation ------------------------------------------------
        if gross_amount == 0 {
            return Err(SettlementError::InvalidAmount);
        }
        if user == provider {
            return Err(SettlementError::SameUserAndProvider);
        }

        // --- Fee split -------------------------------------------------
        let (fee_amount, net_amount) = self.calculate_split(gross_amount);

        // --- Deterministic IDs -----------------------------------------
        let mut id_input = Vec::new();
        id_input.extend_from_slice(user.as_bytes());
        id_input.extend_from_slice(provider.as_bytes());
        id_input.extend_from_slice(&gross_amount.to_le_bytes());
        id_input.extend_from_slice(&height.to_le_bytes());
        id_input.extend_from_slice(&timestamp.to_le_bytes());
        id_input.extend_from_slice(&(self.records.len() as u64).to_le_bytes());
        let id = Hash::hash_data(&id_input);

        let mut burn_input = id_input.clone();
        burn_input.push(0x42); // 'B' marker
        let burn_tx = Hash::hash_data(&burn_input);

        let mut mint_input = id_input.clone();
        mint_input.push(0x4D); // 'M' marker
        let mint_tx = Hash::hash_data(&mint_input);

        // --- Record ----------------------------------------------------
        let record = SettlementRecord {
            id,
            user,
            provider,
            gross_amount,
            fee_amount,
            net_amount,
            burn_tx,
            mint_tx,
            height,
            timestamp,
            service_type,
            status: SettlementStatus::Completed,
        };

        let index = self.records.len();
        self.records_by_user
            .entry(user)
            .or_default()
            .push(index);
        self.records_by_provider
            .entry(provider)
            .or_default()
            .push(index);

        self.total_settled = self.total_settled.saturating_add(gross_amount);
        self.total_fees = self.total_fees.saturating_add(fee_amount);

        self.records.push(record.clone());
        Ok(record)
    }

    /// Process a batch of settlements in one call.
    ///
    /// Each entry is `(user, provider, gross_amount, service_type)`.
    /// `height` and `timestamp` apply to all entries in the batch.
    /// Returns one result per input entry, in order.
    pub fn batch_settle(
        &mut self,
        settlements: Vec<(Address, Address, Amount, ServiceType)>,
        height: BlockHeight,
        timestamp: Timestamp,
    ) -> Vec<Result<SettlementRecord, SettlementError>> {
        settlements
            .into_iter()
            .map(|(user, provider, amount, service_type)| {
                self.settle(user, provider, amount, service_type, height, timestamp)
            })
            .collect()
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    /// Look up a single settlement record by its ID.
    pub fn get_record(&self, id: &Hash) -> Option<&SettlementRecord> {
        self.records.iter().find(|r| &r.id == id)
    }

    /// All settlement records where `user` is the payer.
    pub fn get_user_records(&self, user: &Address) -> Vec<&SettlementRecord> {
        self.records_by_user
            .get(user)
            .map(|indices| indices.iter().map(|&i| &self.records[i]).collect())
            .unwrap_or_default()
    }

    /// All settlement records where `provider` is the recipient.
    pub fn get_provider_records(&self, provider: &Address) -> Vec<&SettlementRecord> {
        self.records_by_provider
            .get(provider)
            .map(|indices| indices.iter().map(|&i| &self.records[i]).collect())
            .unwrap_or_default()
    }

    /// Running total of all gross amounts settled.
    pub fn get_total_settled(&self) -> Amount {
        self.total_settled
    }

    /// Running total of all protocol fees collected.
    pub fn get_total_fees(&self) -> Amount {
        self.total_fees
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
        Address::from([0x11; 20])
    }

    fn provider() -> Address {
        Address::from([0x22; 20])
    }

    fn provider2() -> Address {
        Address::from([0x33; 20])
    }

    fn engine() -> SettlementEngine {
        SettlementEngine::new(PROTOCOL_FEE_PERCENT)
    }

    const GROSS: Amount = 1_000_000;

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
}
