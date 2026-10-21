use crate::settlement::{SettlementEngine, SettlementRecord};
use crate::types::{Amount, BlockHeight, Timestamp};
use std::collections::{HashSet, VecDeque};

use super::types::{BillingEvent, BridgeConfig, BridgeError, BridgeStats};

// ============================================================================
// SettlementBridge
// ============================================================================

/// Accumulates `BillingEvent`s (typically from a NATS subscriber) and
/// periodically batches them into on-chain `SettlementRecord`s.
///
/// The bridge handles:
/// - **Deduplication** — recent event IDs are tracked in a sliding window.
/// - **Batching** — events are drained when either the block interval elapses
///   or the queue reaches `max_batch_size`.
/// - **Failure tracking** — events that fail settlement are held in
///   `failed_events` for later retry.
pub struct SettlementBridge {
    /// Runtime configuration.
    pub config: BridgeConfig,
    /// Events waiting to be settled.
    pub event_queue: Vec<BillingEvent>,
    /// Sliding window of recently seen event IDs (deduplication).
    pub processed_events: HashSet<String>,
    /// Ordered queue of processed event IDs so the oldest can be evicted.
    dedup_order: VecDeque<String>,
    /// Total settlement batches created.
    pub batches_created: u64,
    /// Total billing events that have been processed into batches.
    pub events_processed: u64,
    /// Cumulative gross amount forwarded to the settlement engine.
    pub total_settled: Amount,
    /// Block height at which the last batch was created (0 = never).
    pub last_batch_height: BlockHeight,
    /// Events that failed settlement, paired with the reason string.
    pub failed_events: Vec<(BillingEvent, String)>,
}

impl SettlementBridge {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Create a new bridge with the given configuration.
    pub fn new(config: BridgeConfig) -> Self {
        SettlementBridge {
            config,
            event_queue: Vec::new(),
            processed_events: HashSet::new(),
            dedup_order: VecDeque::new(),
            batches_created: 0,
            events_processed: 0,
            total_settled: 0,
            last_batch_height: 0,
            failed_events: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Ingestion
    // -----------------------------------------------------------------------

    /// Accept a new `BillingEvent` into the queue.
    ///
    /// Rejects events that are:
    /// - Already seen (duplicate `event_id`).
    /// - Carrying a zero amount.
    /// - Invalid (user == provider).
    pub fn ingest_event(&mut self, event: BillingEvent) -> Result<(), BridgeError> {
        // Zero-amount guard
        if event.amount == 0 {
            return Err(BridgeError::ZeroAmount);
        }

        // Self-settlement guard
        if event.user == event.provider {
            return Err(BridgeError::InvalidEvent(
                "user and provider addresses must differ".to_string(),
            ));
        }

        // Deduplication check
        if self.processed_events.contains(&event.event_id) {
            return Err(BridgeError::DuplicateEvent(event.event_id.clone()));
        }

        // Record event ID in the dedup window
        self.processed_events.insert(event.event_id.clone());
        self.dedup_order.push_back(event.event_id.clone());

        // Evict oldest entries if the window is full
        while self.dedup_order.len() > self.config.dedup_window {
            if let Some(oldest) = self.dedup_order.pop_front() {
                self.processed_events.remove(&oldest);
            }
        }

        self.event_queue.push(event);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Batching
    // -----------------------------------------------------------------------

    /// Return `true` when it is time to create a new settlement batch.
    ///
    /// Triggers when:
    /// - At least `batch_interval_blocks` have elapsed since the last batch, **or**
    /// - The queue has reached `max_batch_size`.
    pub fn should_batch(&self, current_height: BlockHeight) -> bool {
        if self.event_queue.is_empty() {
            return false;
        }

        let interval_elapsed =
            current_height >= self.last_batch_height + self.config.batch_interval_blocks;
        let queue_full = self.event_queue.len() >= self.config.max_batch_size;

        interval_elapsed || queue_full
    }

    /// Drain the event queue and convert it into `SettlementRecord`s.
    ///
    /// Uses an internal `SettlementEngine` (configured with the bridge's
    /// `fee_rate_bps`) to process each event.  Events that fail settlement
    /// are moved to `failed_events`.
    ///
    /// Returns `Err(BridgeError::BatchFailed)` if the queue is empty.
    pub fn create_batch(
        &mut self,
        current_height: BlockHeight,
        timestamp: Timestamp,
    ) -> Result<Vec<SettlementRecord>, BridgeError> {
        if self.event_queue.is_empty() {
            return Err(BridgeError::BatchFailed("nothing to batch".to_string()));
        }

        // Drain the queue
        let events: Vec<BillingEvent> = self.event_queue.drain(..).collect();

        let mut engine = SettlementEngine::new(self.config.fee_rate_bps);
        let mut records = Vec::with_capacity(events.len());

        for event in events {
            match engine.settle(
                event.user,
                event.provider,
                event.amount,
                event.service_type.clone(),
                current_height,
                timestamp,
            ) {
                Ok(record) => {
                    self.total_settled = self.total_settled.saturating_add(event.amount);
                    self.events_processed += 1;
                    records.push(record);
                }
                Err(e) => {
                    self.failed_events
                        .push((event, format!("{}", e)));
                }
            }
        }

        self.batches_created += 1;
        self.last_batch_height = current_height;

        Ok(records)
    }

    // -----------------------------------------------------------------------
    // Queue helpers
    // -----------------------------------------------------------------------

    /// Current number of events waiting in the queue.
    pub fn get_queue_size(&self) -> usize {
        self.event_queue.len()
    }

    /// Snapshot of bridge statistics.
    pub fn get_stats(&self) -> BridgeStats {
        BridgeStats {
            events_processed: self.events_processed,
            batches_created: self.batches_created,
            total_settled: self.total_settled,
            queue_size: self.event_queue.len(),
            failed_count: self.failed_events.len(),
        }
    }

    /// Read-only view of events that failed settlement.
    pub fn get_failed_events(&self) -> &[(BillingEvent, String)] {
        &self.failed_events
    }

    /// Forcibly drain the queue without creating a settlement batch.
    ///
    /// Useful for shutdown / reset scenarios.
    pub fn flush_queue(&mut self) -> Vec<BillingEvent> {
        self.event_queue.drain(..).collect()
    }

    /// Move all failed events back into the queue for a retry attempt.
    ///
    /// Returns the events that were re-queued (clones).
    pub fn retry_failed(&mut self) -> Vec<BillingEvent> {
        let retry_events: Vec<BillingEvent> = self
            .failed_events
            .drain(..)
            .map(|(event, _reason)| event)
            .collect();

        let requeued = retry_events.clone();
        for event in retry_events {
            self.event_queue.push(event);
        }
        requeued
    }
}

impl Default for SettlementBridge {
    fn default() -> Self {
        Self::new(BridgeConfig::default())
    }
}
