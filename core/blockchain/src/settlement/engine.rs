use super::types::*;
use crate::types::{Address, Amount, BlockHeight, Hash, Timestamp};
use std::collections::HashMap;

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
