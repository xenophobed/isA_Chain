use super::*;
use crate::credit_bridge::types::{BridgeEventStatus, CreditBridgeError};

#[test]
fn test_complete_event() {
    let mut bridge = setup();

    let id = bridge
        .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 10)
        .unwrap();

    bridge.complete_event(&id).unwrap();

    let event = bridge.get_event(&id).unwrap();
    assert_eq!(event.status, BridgeEventStatus::Completed);
}

#[test]
fn test_fail_event() {
    let mut bridge = setup();

    let id = bridge
        .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 10)
        .unwrap();

    bridge.fail_event(&id, "oracle timeout".to_string()).unwrap();

    let event = bridge.get_event(&id).unwrap();
    assert_eq!(
        event.status,
        BridgeEventStatus::Failed("oracle timeout".to_string())
    );
}

#[test]
fn test_complete_updates_stats() {
    let mut bridge = setup();

    let id1 = bridge
        .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 10)
        .unwrap();
    let id2 = bridge
        .initiate_redemption(user2(), 200, 4_000_000, 11)
        .unwrap();

    bridge.complete_event(&id1).unwrap();
    bridge.complete_event(&id2).unwrap();

    let (credits, isa) = bridge.get_stats();
    assert_eq!(credits, HUNDRED_CREDITS + 200);
    assert_eq!(isa, TWO_ISA + 4_000_000);
}

#[test]
fn test_pending_count() {
    let mut bridge = setup();

    assert_eq!(bridge.get_pending_count(), 0);

    let id1 = bridge
        .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 1)
        .unwrap();
    assert_eq!(bridge.get_pending_count(), 1);

    let _id2 = bridge
        .initiate_purchase(user2(), TWO_ISA, HUNDRED_CREDITS, 2)
        .unwrap();
    assert_eq!(bridge.get_pending_count(), 2);

    bridge.complete_event(&id1).unwrap();
    assert_eq!(bridge.get_pending_count(), 1);
}

#[test]
fn test_pending_count_on_failure() {
    let mut bridge = setup();

    let id = bridge
        .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 1)
        .unwrap();
    assert_eq!(bridge.get_pending_count(), 1);

    bridge.fail_event(&id, "network error".to_string()).unwrap();
    assert_eq!(bridge.get_pending_count(), 0);
}

#[test]
fn test_already_completed_error() {
    let mut bridge = setup();

    let id = bridge
        .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 1)
        .unwrap();

    bridge.complete_event(&id).unwrap();

    // Completing again must fail
    assert_eq!(
        bridge.complete_event(&id),
        Err(CreditBridgeError::AlreadyCompleted)
    );

    // Failing a completed event must also fail
    assert_eq!(
        bridge.fail_event(&id, "late".to_string()),
        Err(CreditBridgeError::AlreadyCompleted)
    );
}

#[test]
fn test_stats_only_count_completed() {
    let mut bridge = setup();

    let id1 = bridge
        .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 1)
        .unwrap();
    let id2 = bridge
        .initiate_purchase(user2(), TWO_ISA, HUNDRED_CREDITS, 2)
        .unwrap();

    // Complete only the first
    bridge.complete_event(&id1).unwrap();

    // Fail the second — should NOT count in stats
    bridge
        .fail_event(&id2, "rejected".to_string())
        .unwrap();

    let (credits, isa) = bridge.get_stats();
    assert_eq!(credits, HUNDRED_CREDITS); // only id1
    assert_eq!(isa, TWO_ISA);             // only id1
}

#[test]
fn test_get_stats_initial() {
    let bridge = setup();
    assert_eq!(bridge.get_stats(), (0, 0));
    assert_eq!(bridge.get_pending_count(), 0);
}
