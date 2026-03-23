use crate::types::{Address, Amount, BlockHeight, Hash};
use crate::types::constants::PROTOCOL_FEE_PERCENT;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ============================================================================
// BatchStatus
// ============================================================================

/// Lifecycle state of a batch settlement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchStatus {
    /// Batch has been created and is awaiting processing.
    Pending,
    /// Batch is currently being processed.
    Processing,
    /// All settlements in the batch completed successfully.
    Completed,
    /// Batch completed with some successes and some failures.
    PartiallyCompleted { succeeded: usize, failed: usize },
    /// Batch failed entirely; reason attached.
    Failed(String),
}

// ============================================================================
// BatchSettlementError
// ============================================================================

/// Errors that can arise during batch settlement operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BatchSettlementError {
    #[error("Cannot create a batch with no settlements")]
    EmptyBatch,

    #[error("Batch size {actual} exceeds maximum allowed {max}")]
    BatchTooLarge { max: usize, actual: usize },

    #[error("Invalid settlement: {0}")]
    InvalidSettlement(String),

    #[error("Channel not found: {0:?}")]
    ChannelNotFound(Hash),

    #[error("Channel already settled: {0:?}")]
    AlreadySettled(Hash),
}

// ============================================================================
// ChannelSettlement
// ============================================================================

/// A single channel's contribution to a batch settlement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelSettlement {
    /// The channel being settled.
    pub channel_id: Hash,
    /// Sender (payer) address.
    pub sender: Address,
    /// Receiver (payee) address.
    pub receiver: Address,
    /// Amount transferred from sender to receiver.
    pub amount: Amount,
    /// Protocol fee deducted from `amount`.
    pub fee: Amount,
    /// Channel nonce at the time of settlement (monotonically increasing).
    pub final_nonce: u64,
}

// ============================================================================
// BatchSettlement
// ============================================================================

/// An aggregated settlement of multiple payment channels written to the chain
/// in a single operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchSettlement {
    /// Unique batch identifier (deterministic hash of all settlements).
    pub id: Hash,
    /// Individual channel settlements included in this batch.
    pub settlements: Vec<ChannelSettlement>,
    /// Sum of all `amount` fields across settlements.
    pub total_amount: Amount,
    /// Sum of all `fee` fields across settlements.
    pub total_fees: Amount,
    /// Block height at which the batch was created.
    pub height: BlockHeight,
    /// Current lifecycle status.
    pub status: BatchStatus,
}

// ============================================================================
// BatchProcessor
// ============================================================================

/// Processes payment channel settlements in periodic batches, writing the
/// aggregate result to the chain rather than one transaction per channel.
///
/// Double-settlement is prevented via `settled_channels`: any channel ID that
/// appears in a successfully processed batch is permanently recorded and
/// rejected if submitted again.
pub struct BatchProcessor {
    /// All batches ever submitted, in order of creation.
    pub batches: Vec<BatchSettlement>,
    /// Set of channel IDs that have already been settled.
    pub settled_channels: HashSet<Hash>,
    /// Maximum number of channel settlements per batch.
    pub max_batch_size: usize,
    /// Protocol fee rate in basis points (e.g. 250 = 2.5%).
    pub fee_rate_bps: u32,
    /// Total number of batches that have been processed to `Completed` or
    /// `PartiallyCompleted` status.
    pub total_batches_processed: u64,
    /// Cumulative amount settled across all processed batches.
    pub total_settled: Amount,
}

impl BatchProcessor {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Create a new `BatchProcessor`.
    ///
    /// - `max_batch_size`: upper limit on settlements per batch (default 100).
    /// - `fee_rate_bps`: protocol fee in basis points (default `PROTOCOL_FEE_PERCENT`).
    pub fn new(max_batch_size: usize, fee_rate_bps: u32) -> Self {
        BatchProcessor {
            batches: Vec::new(),
            settled_channels: HashSet::new(),
            max_batch_size,
            fee_rate_bps,
            total_batches_processed: 0,
            total_settled: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Fee helper
    // -----------------------------------------------------------------------

    /// Calculate the protocol fee for `amount` using `fee_rate_bps`.
    ///
    /// Uses integer (floor) arithmetic: `fee = amount * bps / 10_000`.
    pub fn calculate_fee(&self, amount: Amount) -> Amount {
        amount.saturating_mul(self.fee_rate_bps as u128) / 10_000
    }

    // -----------------------------------------------------------------------
    // Batch creation
    // -----------------------------------------------------------------------

    /// Build a new `BatchSettlement` from a list of raw channel data tuples.
    ///
    /// Each tuple is `(channel_id, sender, receiver, amount, nonce)`.
    ///
    /// Validation checks:
    /// - The input list must be non-empty.
    /// - The number of entries must not exceed `max_batch_size`.
    /// - Each amount must be greater than zero.
    /// - No channel ID may appear more than once in the same batch.
    /// - No channel ID may already be in `settled_channels`.
    ///
    /// On success the new batch is appended to `self.batches` and returned
    /// as a reference.
    pub fn create_batch(
        &mut self,
        settlements: Vec<(Hash, Address, Address, Amount, u64)>,
        height: BlockHeight,
    ) -> Result<BatchSettlement, BatchSettlementError> {
        // --- Guard: non-empty -----------------------------------------
        if settlements.is_empty() {
            return Err(BatchSettlementError::EmptyBatch);
        }

        // --- Guard: size limit ----------------------------------------
        if settlements.len() > self.max_batch_size {
            return Err(BatchSettlementError::BatchTooLarge {
                max: self.max_batch_size,
                actual: settlements.len(),
            });
        }

        // --- Per-entry validation and fee calculation -----------------
        let mut channel_settlements: Vec<ChannelSettlement> = Vec::with_capacity(settlements.len());
        let mut seen_in_batch: HashSet<Hash> = HashSet::new();
        let mut total_amount: Amount = 0;
        let mut total_fees: Amount = 0;

        for (channel_id, sender, receiver, amount, nonce) in &settlements {
            // Amount must be positive
            if *amount == 0 {
                return Err(BatchSettlementError::InvalidSettlement(format!(
                    "channel {:?} has zero amount",
                    channel_id
                )));
            }

            // No duplicates within this batch
            if !seen_in_batch.insert(*channel_id) {
                return Err(BatchSettlementError::InvalidSettlement(format!(
                    "channel {:?} appears more than once in the batch",
                    channel_id
                )));
            }

            // No channel that was already settled
            if self.settled_channels.contains(channel_id) {
                return Err(BatchSettlementError::AlreadySettled(*channel_id));
            }

            let fee = self.calculate_fee(*amount);
            total_amount = total_amount.saturating_add(*amount);
            total_fees = total_fees.saturating_add(fee);

            channel_settlements.push(ChannelSettlement {
                channel_id: *channel_id,
                sender: *sender,
                receiver: *receiver,
                amount: *amount,
                fee,
                final_nonce: *nonce,
            });
        }

        // --- Deterministic batch ID -----------------------------------
        let mut id_input: Vec<u8> = Vec::new();
        id_input.extend_from_slice(&height.to_le_bytes());
        id_input.extend_from_slice(&(self.batches.len() as u64).to_le_bytes());
        for cs in &channel_settlements {
            id_input.extend_from_slice(cs.channel_id.as_bytes());
            id_input.extend_from_slice(&cs.amount.to_le_bytes());
            id_input.extend_from_slice(&cs.final_nonce.to_le_bytes());
        }
        let batch_id = Hash::hash_data(&id_input);

        let batch = BatchSettlement {
            id: batch_id,
            settlements: channel_settlements,
            total_amount,
            total_fees,
            height,
            status: BatchStatus::Pending,
        };

        self.batches.push(batch.clone());
        Ok(batch)
    }

    // -----------------------------------------------------------------------
    // Batch processing
    // -----------------------------------------------------------------------

    /// Mark a pending batch as `Completed`.
    ///
    /// All channel IDs in the batch are added to `settled_channels` to prevent
    /// future double-settlement.  Running totals are updated and
    /// `total_batches_processed` is incremented.
    ///
    /// Returns a reference to the updated `BatchSettlement`.
    pub fn process_batch(
        &mut self,
        batch_id: &Hash,
    ) -> Result<&BatchSettlement, BatchSettlementError> {
        let idx = self
            .batches
            .iter()
            .position(|b| &b.id == batch_id)
            .ok_or(BatchSettlementError::ChannelNotFound(*batch_id))?;

        // Mark every channel as settled and accumulate totals
        let batch = &self.batches[idx];
        let channel_ids: Vec<Hash> = batch.settlements.iter().map(|s| s.channel_id).collect();
        let batch_total = batch.total_amount;

        for cid in channel_ids {
            self.settled_channels.insert(cid);
        }

        self.total_settled = self.total_settled.saturating_add(batch_total);
        self.total_batches_processed += 1;

        // Update status
        self.batches[idx].status = BatchStatus::Completed;

        Ok(&self.batches[idx])
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Return a reference to the batch with the given ID, or `None`.
    pub fn get_batch(&self, id: &Hash) -> Option<&BatchSettlement> {
        self.batches.iter().find(|b| &b.id == id)
    }

    /// Return all batches created at a specific block height.
    pub fn get_batches_at_height(&self, height: BlockHeight) -> Vec<&BatchSettlement> {
        self.batches.iter().filter(|b| b.height == height).collect()
    }

    /// Return `true` if the channel has already been settled.
    pub fn is_channel_settled(&self, channel_id: &Hash) -> bool {
        self.settled_channels.contains(channel_id)
    }

    /// Cumulative amount settled across all processed batches.
    pub fn get_total_settled(&self) -> Amount {
        self.total_settled
    }

    /// Total number of batches that have been successfully processed.
    pub fn get_total_batches(&self) -> u64 {
        self.total_batches_processed
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn sender() -> Address {
        Address::from([0xAA; 20])
    }

    fn receiver() -> Address {
        Address::from([0xBB; 20])
    }

    fn channel_id(seed: u8) -> Hash {
        Hash::hash_data(&[seed; 32])
    }

    /// Build a processor with default settings (max 100, 2.5% fee).
    fn processor() -> BatchProcessor {
        BatchProcessor::new(100, PROTOCOL_FEE_PERCENT)
    }

    /// One valid settlement tuple.
    fn settlement(seed: u8, amount: Amount, nonce: u64) -> (Hash, Address, Address, Amount, u64) {
        (channel_id(seed), sender(), receiver(), amount, nonce)
    }

    const HEIGHT: BlockHeight = 42;

    // -----------------------------------------------------------------------
    // test_create_batch
    // -----------------------------------------------------------------------

    #[test]
    fn test_create_batch() {
        let mut p = processor();
        let batch = p
            .create_batch(vec![settlement(1, 1_000, 1), settlement(2, 2_000, 2)], HEIGHT)
            .unwrap();

        assert_eq!(batch.settlements.len(), 2);
        assert_eq!(batch.total_amount, 3_000);
        assert_eq!(batch.height, HEIGHT);
        assert_eq!(batch.status, BatchStatus::Pending);
        assert_ne!(batch.id, Hash::ZERO);

        // Fee for 1_000 at 250 bps = 25; fee for 2_000 = 50
        assert_eq!(batch.total_fees, 75);
    }

    // -----------------------------------------------------------------------
    // test_empty_batch_fails
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_batch_fails() {
        let mut p = processor();
        let result = p.create_batch(vec![], HEIGHT);
        assert!(matches!(result, Err(BatchSettlementError::EmptyBatch)));
    }

    // -----------------------------------------------------------------------
    // test_batch_too_large
    // -----------------------------------------------------------------------

    #[test]
    fn test_batch_too_large() {
        let mut p = BatchProcessor::new(3, PROTOCOL_FEE_PERCENT);
        let settlements: Vec<_> = (1..=4).map(|i| settlement(i, 100, i as u64)).collect();
        let result = p.create_batch(settlements, HEIGHT);
        assert!(matches!(
            result,
            Err(BatchSettlementError::BatchTooLarge { max: 3, actual: 4 })
        ));
    }

    // -----------------------------------------------------------------------
    // test_process_batch
    // -----------------------------------------------------------------------

    #[test]
    fn test_process_batch() {
        let mut p = processor();
        let batch = p.create_batch(vec![settlement(1, 1_000, 1)], HEIGHT).unwrap();
        let batch_id = batch.id;

        let processed = p.process_batch(&batch_id).unwrap();
        assert_eq!(processed.status, BatchStatus::Completed);
        assert_eq!(p.get_total_batches(), 1);
        assert_eq!(p.get_total_settled(), 1_000);
    }

    // -----------------------------------------------------------------------
    // test_duplicate_channel_fails
    // -----------------------------------------------------------------------

    #[test]
    fn test_duplicate_channel_fails() {
        let mut p = processor();

        // First batch: settle channel 1
        let batch = p.create_batch(vec![settlement(1, 1_000, 1)], HEIGHT).unwrap();
        p.process_batch(&batch.id).unwrap();

        // Channel 1 is now in settled_channels — second batch must fail
        let result = p.create_batch(vec![settlement(1, 500, 2)], HEIGHT + 1);
        assert!(matches!(result, Err(BatchSettlementError::AlreadySettled(_))));
    }

    // -----------------------------------------------------------------------
    // test_duplicate_channel_within_batch_fails
    // -----------------------------------------------------------------------

    #[test]
    fn test_duplicate_channel_within_batch_fails() {
        let mut p = processor();

        // Two entries with the same channel_id in a single batch
        let cid = channel_id(1);
        let dup = vec![
            (cid, sender(), receiver(), 500, 1u64),
            (cid, sender(), receiver(), 500, 2u64),
        ];
        let result = p.create_batch(dup, HEIGHT);
        assert!(matches!(result, Err(BatchSettlementError::InvalidSettlement(_))));
    }

    // -----------------------------------------------------------------------
    // test_fee_calculation
    // -----------------------------------------------------------------------

    #[test]
    fn test_fee_calculation() {
        let p = processor(); // 250 bps = 2.5%

        assert_eq!(p.calculate_fee(1_000_000), 25_000);
        assert_eq!(p.calculate_fee(0), 0);
        // Small amount: 100 * 250 / 10_000 = 2 (floor)
        assert_eq!(p.calculate_fee(100), 2);
        // 1 * 250 / 10_000 = 0 (floor)
        assert_eq!(p.calculate_fee(1), 0);
    }

    // -----------------------------------------------------------------------
    // test_get_batches_at_height
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_batches_at_height() {
        let mut p = processor();
        p.create_batch(vec![settlement(1, 100, 1)], 10).unwrap();
        p.create_batch(vec![settlement(2, 200, 1)], 10).unwrap();
        p.create_batch(vec![settlement(3, 300, 1)], 20).unwrap();

        let at_10 = p.get_batches_at_height(10);
        assert_eq!(at_10.len(), 2);

        let at_20 = p.get_batches_at_height(20);
        assert_eq!(at_20.len(), 1);

        let at_99 = p.get_batches_at_height(99);
        assert!(at_99.is_empty());
    }

    // -----------------------------------------------------------------------
    // test_total_tracking
    // -----------------------------------------------------------------------

    #[test]
    fn test_total_tracking() {
        let mut p = processor();

        let b1 = p.create_batch(vec![settlement(1, 1_000, 1), settlement(2, 2_000, 1)], 1).unwrap();
        let b2 = p.create_batch(vec![settlement(3, 500, 1)], 2).unwrap();

        p.process_batch(&b1.id).unwrap();
        p.process_batch(&b2.id).unwrap();

        assert_eq!(p.get_total_settled(), 3_500);
        assert_eq!(p.get_total_batches(), 2);
    }

    // -----------------------------------------------------------------------
    // test_partially_completed
    // -----------------------------------------------------------------------

    #[test]
    fn test_partially_completed() {
        let mut p = processor();
        let batch = p
            .create_batch(vec![settlement(1, 100, 1), settlement(2, 200, 1)], HEIGHT)
            .unwrap();
        let batch_id = batch.id;

        // Manually set PartiallyCompleted to confirm the enum round-trips
        let idx = p.batches.iter().position(|b| b.id == batch_id).unwrap();
        p.batches[idx].status = BatchStatus::PartiallyCompleted {
            succeeded: 1,
            failed: 1,
        };

        let b = p.get_batch(&batch_id).unwrap();
        assert!(matches!(
            b.status,
            BatchStatus::PartiallyCompleted { succeeded: 1, failed: 1 }
        ));
    }

    // -----------------------------------------------------------------------
    // test_multiple_batches
    // -----------------------------------------------------------------------

    #[test]
    fn test_multiple_batches() {
        let mut p = processor();

        // Create and process 5 separate single-channel batches
        for seed in 1u8..=5 {
            let batch = p
                .create_batch(vec![settlement(seed, 1_000, 1)], HEIGHT + seed as u64)
                .unwrap();
            p.process_batch(&batch.id).unwrap();
        }

        assert_eq!(p.get_total_batches(), 5);
        assert_eq!(p.get_total_settled(), 5_000);
        assert_eq!(p.settled_channels.len(), 5);

        // All 5 channel IDs should be marked settled
        for seed in 1u8..=5 {
            assert!(p.is_channel_settled(&channel_id(seed)));
        }
        // Channel 6 was never settled
        assert!(!p.is_channel_settled(&channel_id(6)));
    }

    // -----------------------------------------------------------------------
    // test_zero_amount_settlement_fails
    // -----------------------------------------------------------------------

    #[test]
    fn test_zero_amount_settlement_fails() {
        let mut p = processor();
        let result = p.create_batch(vec![settlement(1, 0, 1)], HEIGHT);
        assert!(matches!(result, Err(BatchSettlementError::InvalidSettlement(_))));
    }

    // -----------------------------------------------------------------------
    // test_is_channel_settled
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_channel_settled() {
        let mut p = processor();
        let cid = channel_id(42);

        assert!(!p.is_channel_settled(&cid));

        let batch = p
            .create_batch(vec![(cid, sender(), receiver(), 500, 1)], HEIGHT)
            .unwrap();
        // Not yet settled until process_batch is called
        assert!(!p.is_channel_settled(&cid));

        p.process_batch(&batch.id).unwrap();
        assert!(p.is_channel_settled(&cid));
    }
}
