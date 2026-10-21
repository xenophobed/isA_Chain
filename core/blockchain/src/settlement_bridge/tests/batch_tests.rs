use super::*;
use crate::settlement_bridge::types::{BridgeConfig, BridgeError};

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

#[test]
fn test_empty_batch() {
    let mut b = bridge();
    let result = b.create_batch(20, 20_000);
    assert!(matches!(result, Err(BridgeError::BatchFailed(_))));
}

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
