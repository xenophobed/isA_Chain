use crate::types::{Address, Amount, BlockHeight, Hash};
use std::collections::HashSet;

use super::types::{BatchSettlement, BatchSettlementError, BatchStatus, ChannelSettlement};

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
