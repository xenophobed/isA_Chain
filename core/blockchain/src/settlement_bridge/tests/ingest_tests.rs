use super::*;
use crate::settlement::ServiceType;
use crate::settlement_bridge::types::{BridgeConfig, BridgeError};
use std::collections::HashMap;

#[test]
fn test_ingest_event() {
    let mut b = bridge();
    assert!(b.ingest_event(make_event("evt-1", 1_000)).is_ok());
    assert_eq!(b.get_queue_size(), 1);
}

#[test]
fn test_duplicate_event_rejected() {
    let mut b = bridge();
    b.ingest_event(make_event("evt-1", 1_000)).unwrap();
    let result = b.ingest_event(make_event("evt-1", 500));
    assert!(matches!(result, Err(BridgeError::DuplicateEvent(_))));
    // Queue still has exactly one event
    assert_eq!(b.get_queue_size(), 1);
}

#[test]
fn test_zero_amount_rejected() {
    let mut b = bridge();
    let result = b.ingest_event(make_event("evt-zero", 0));
    assert!(matches!(result, Err(BridgeError::ZeroAmount)));
    assert_eq!(b.get_queue_size(), 0);
}

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
