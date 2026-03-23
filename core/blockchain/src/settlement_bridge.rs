use crate::settlement::{ServiceType, SettlementRecord, SettlementEngine};
use crate::types::{Address, Amount, BlockHeight, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

// ============================================================================
// BillingEvent
// ============================================================================

/// A single billing event received from a NATS subscriber, representing one
/// unit of work (inference call, tool execution, etc.) that will be settled
/// on-chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillingEvent {
    /// Unique billing event ID (used for deduplication).
    pub event_id: String,
    /// Address of the user being charged.
    pub user: Address,
    /// Address of the service provider being paid.
    pub provider: Address,
    /// Amount to settle (gross, before protocol fee).
    pub amount: Amount,
    /// Which isA service generated this billing event.
    pub service_type: ServiceType,
    /// Unix timestamp (milliseconds) when the event was generated.
    pub timestamp: Timestamp,
    /// Provider wallet address for receiving the net payment.
    pub provider_wallet: Address,
    /// Service-specific metadata (e.g. model name, tool ID, job ID).
    pub metadata: HashMap<String, String>,
}

// ============================================================================
// BridgeConfig
// ============================================================================

/// Configuration for the `SettlementBridge`.
#[derive(Clone, Debug)]
pub struct BridgeConfig {
    /// Settle pending events every N blocks (default 20 ≈ 60 s at 3 s blocks).
    pub batch_interval_blocks: u64,
    /// Maximum number of billing events per settlement batch (default 100).
    pub max_batch_size: usize,
    /// Number of recent event IDs to remember for deduplication (default 10 000).
    pub dedup_window: usize,
    /// Protocol fee in basis points deducted during settlement (default 250 = 2.5 %).
    pub fee_rate_bps: u32,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        BridgeConfig {
            batch_interval_blocks: 20,
            max_batch_size: 100,
            dedup_window: 10_000,
            fee_rate_bps: 250,
        }
    }
}

// ============================================================================
// BridgeError
// ============================================================================

/// Errors that can arise while operating the `SettlementBridge`.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BridgeError {
    #[error("Duplicate billing event: {0}")]
    DuplicateEvent(String),

    #[error("Invalid billing event: {0}")]
    InvalidEvent(String),

    #[error("Batch settlement failed: {0}")]
    BatchFailed(String),

    #[error("Event queue is full")]
    QueueFull,

    #[error("Amount must be greater than zero")]
    ZeroAmount,
}

// ============================================================================
// BridgeStats
// ============================================================================

/// Summary statistics for the `SettlementBridge`.
#[derive(Clone, Debug)]
pub struct BridgeStats {
    /// Total billing events successfully ingested and processed.
    pub events_processed: u64,
    /// Total settlement batches created.
    pub batches_created: u64,
    /// Cumulative gross amount sent to the settlement engine.
    pub total_settled: Amount,
    /// Number of events currently waiting in the queue.
    pub queue_size: usize,
    /// Number of events that failed settlement and are held for retry.
    pub failed_count: usize,
}

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

    /// Create a bridge with default configuration.
    pub fn default() -> Self {
        Self::new(BridgeConfig::default())
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn user() -> Address {
        Address::from([0x11; 20])
    }

    fn provider() -> Address {
        Address::from([0x22; 20])
    }

    fn make_event(id: &str, amount: Amount) -> BillingEvent {
        BillingEvent {
            event_id: id.to_string(),
            user: user(),
            provider: provider(),
            amount,
            service_type: ServiceType::ModelInference,
            timestamp: 1_000,
            provider_wallet: provider(),
            metadata: HashMap::new(),
        }
    }

    fn bridge() -> SettlementBridge {
        SettlementBridge::default()
    }

    // -----------------------------------------------------------------------
    // test_ingest_event
    // -----------------------------------------------------------------------

    #[test]
    fn test_ingest_event() {
        let mut b = bridge();
        assert!(b.ingest_event(make_event("evt-1", 1_000)).is_ok());
        assert_eq!(b.get_queue_size(), 1);
    }

    // -----------------------------------------------------------------------
    // test_duplicate_event_rejected
    // -----------------------------------------------------------------------

    #[test]
    fn test_duplicate_event_rejected() {
        let mut b = bridge();
        b.ingest_event(make_event("evt-1", 1_000)).unwrap();
        let result = b.ingest_event(make_event("evt-1", 500));
        assert!(matches!(result, Err(BridgeError::DuplicateEvent(_))));
        // Queue still has exactly one event
        assert_eq!(b.get_queue_size(), 1);
    }

    // -----------------------------------------------------------------------
    // test_zero_amount_rejected
    // -----------------------------------------------------------------------

    #[test]
    fn test_zero_amount_rejected() {
        let mut b = bridge();
        let result = b.ingest_event(make_event("evt-zero", 0));
        assert!(matches!(result, Err(BridgeError::ZeroAmount)));
        assert_eq!(b.get_queue_size(), 0);
    }

    // -----------------------------------------------------------------------
    // test_should_batch_by_interval
    // -----------------------------------------------------------------------

    #[test]
    fn test_should_batch_by_interval() {
        let mut b = bridge(); // batch_interval_blocks = 20
        b.ingest_event(make_event("evt-1", 1_000)).unwrap();

        // Block 0 — last_batch_height is 0, 0 >= 0 + 20 is false
        assert!(!b.should_batch(0));
        // Block 19 — not yet
        assert!(!b.should_batch(19));
        // Block 20 — interval elapsed
        assert!(b.should_batch(20));
        // Block 100 — well past interval
        assert!(b.should_batch(100));
    }

    // -----------------------------------------------------------------------
    // test_should_batch_by_size
    // -----------------------------------------------------------------------

    #[test]
    fn test_should_batch_by_size() {
        let config = BridgeConfig {
            batch_interval_blocks: 1_000_000, // very large — won't trigger by time
            max_batch_size: 3,
            dedup_window: 10_000,
            fee_rate_bps: 250,
        };
        let mut b = SettlementBridge::new(config);

        b.ingest_event(make_event("e1", 100)).unwrap();
        b.ingest_event(make_event("e2", 100)).unwrap();
        // 2 < 3 — not full yet
        assert!(!b.should_batch(0));

        b.ingest_event(make_event("e3", 100)).unwrap();
        // 3 >= 3 — trigger by size
        assert!(b.should_batch(0));
    }

    // -----------------------------------------------------------------------
    // test_create_batch
    // -----------------------------------------------------------------------

    #[test]
    fn test_create_batch() {
        let mut b = bridge();
        b.ingest_event(make_event("e1", 1_000)).unwrap();
        b.ingest_event(make_event("e2", 2_000)).unwrap();

        let records = b.create_batch(20, 20_000).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(b.get_queue_size(), 0);
        assert_eq!(b.batches_created, 1);
        assert_eq!(b.events_processed, 2);
        assert_eq!(b.total_settled, 3_000);
        assert_eq!(b.last_batch_height, 20);
    }

    // -----------------------------------------------------------------------
    // test_empty_batch
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_batch() {
        let mut b = bridge();
        let result = b.create_batch(20, 20_000);
        assert!(matches!(result, Err(BridgeError::BatchFailed(_))));
    }

    // -----------------------------------------------------------------------
    // test_stats
    // -----------------------------------------------------------------------

    #[test]
    fn test_stats() {
        let mut b = bridge();
        b.ingest_event(make_event("e1", 500)).unwrap();
        b.ingest_event(make_event("e2", 500)).unwrap();

        let stats = b.get_stats();
        assert_eq!(stats.queue_size, 2);
        assert_eq!(stats.events_processed, 0);
        assert_eq!(stats.batches_created, 0);

        b.create_batch(20, 1_000).unwrap();
        let stats = b.get_stats();
        assert_eq!(stats.queue_size, 0);
        assert_eq!(stats.events_processed, 2);
        assert_eq!(stats.batches_created, 1);
        assert_eq!(stats.total_settled, 1_000);
        assert_eq!(stats.failed_count, 0);
    }

    // -----------------------------------------------------------------------
    // test_flush_queue
    // -----------------------------------------------------------------------

    #[test]
    fn test_flush_queue() {
        let mut b = bridge();
        b.ingest_event(make_event("e1", 100)).unwrap();
        b.ingest_event(make_event("e2", 200)).unwrap();

        let flushed = b.flush_queue();
        assert_eq!(flushed.len(), 2);
        assert_eq!(b.get_queue_size(), 0);
        // No batches were created
        assert_eq!(b.batches_created, 0);
    }

    // -----------------------------------------------------------------------
    // test_retry_failed
    // -----------------------------------------------------------------------

    #[test]
    fn test_retry_failed() {
        let mut b = bridge();

        // Manually inject a failed event
        let evt = make_event("e-failed", 1_000);
        b.failed_events.push((evt.clone(), "simulated failure".to_string()));

        assert_eq!(b.get_failed_events().len(), 1);
        assert_eq!(b.get_queue_size(), 0);

        let requeued = b.retry_failed();
        assert_eq!(requeued.len(), 1);
        assert_eq!(b.get_failed_events().len(), 0);
        assert_eq!(b.get_queue_size(), 1);
    }

    // -----------------------------------------------------------------------
    // test_dedup_window
    // -----------------------------------------------------------------------

    #[test]
    fn test_dedup_window() {
        let config = BridgeConfig {
            batch_interval_blocks: 20,
            max_batch_size: 100,
            dedup_window: 3, // tiny window for testing
            fee_rate_bps: 250,
        };
        let mut b = SettlementBridge::new(config);

        // Fill the dedup window with 3 events
        b.ingest_event(make_event("e1", 100)).unwrap();
        b.ingest_event(make_event("e2", 100)).unwrap();
        b.ingest_event(make_event("e3", 100)).unwrap();
        // Window is [e1, e2, e3]

        // Adding e4 evicts e1 from the window
        b.ingest_event(make_event("e4", 100)).unwrap();
        // Window is now [e2, e3, e4]

        // e1 has been evicted — it should be re-accepted
        assert!(b.ingest_event(make_event("e1", 100)).is_ok());
        // After re-adding e1, window was [e2, e3, e4, e1] → evict e2 → [e3, e4, e1]

        // e3 is still in the window — should be rejected
        assert!(matches!(
            b.ingest_event(make_event("e3", 100)),
            Err(BridgeError::DuplicateEvent(_))
        ));

        // e2 was evicted when e1 was re-added — so e2 should be re-accepted
        assert!(b.ingest_event(make_event("e2", 100)).is_ok());
    }

    // -----------------------------------------------------------------------
    // test_multiple_batches
    // -----------------------------------------------------------------------

    #[test]
    fn test_multiple_batches() {
        let mut b = bridge();

        // First batch
        b.ingest_event(make_event("e1", 1_000)).unwrap();
        b.ingest_event(make_event("e2", 2_000)).unwrap();
        b.create_batch(20, 1_000).unwrap();

        // Second batch
        b.ingest_event(make_event("e3", 500)).unwrap();
        b.create_batch(40, 2_000).unwrap();

        let stats = b.get_stats();
        assert_eq!(stats.batches_created, 2);
        assert_eq!(stats.events_processed, 3);
        assert_eq!(stats.total_settled, 3_500);
        assert_eq!(stats.queue_size, 0);
    }

    // -----------------------------------------------------------------------
    // test_invalid_event_same_user_provider
    // -----------------------------------------------------------------------

    #[test]
    fn test_invalid_event_same_user_provider() {
        let mut b = bridge();
        let bad_event = BillingEvent {
            event_id: "e-bad".to_string(),
            user: user(),
            provider: user(), // same as user — invalid
            amount: 1_000,
            service_type: ServiceType::AgentRuntime,
            timestamp: 1_000,
            provider_wallet: user(),
            metadata: HashMap::new(),
        };
        let result = b.ingest_event(bad_event);
        assert!(matches!(result, Err(BridgeError::InvalidEvent(_))));
    }
}
