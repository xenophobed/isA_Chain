use super::*;
use crate::credit_bridge::types::{BridgeDirection, BridgeEventStatus, BridgeEventType, CreditBridgeError};
use crate::types::Hash;

#[test]
fn test_initiate_purchase() {
    let mut bridge = setup();

    let id = bridge
        .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 10)
        .unwrap();

    let event = bridge.get_event(&id).unwrap();
    assert_eq!(event.user, user());
    assert_eq!(event.isa_amount, TWO_ISA);
    assert_eq!(event.credit_amount, HUNDRED_CREDITS);
    assert_eq!(event.event_type, BridgeEventType::CreditPurchase);
    assert_eq!(event.direction, BridgeDirection::OnChainToOffChain);
    assert_eq!(event.height, 10);
    assert_eq!(event.status, BridgeEventStatus::Initiated);
}

#[test]
fn test_initiate_redemption() {
    let mut bridge = setup();

    let id = bridge
        .initiate_redemption(user(), HUNDRED_CREDITS, TWO_ISA, 20)
        .unwrap();

    let event = bridge.get_event(&id).unwrap();
    assert_eq!(event.user, user());
    assert_eq!(event.credit_amount, HUNDRED_CREDITS);
    assert_eq!(event.isa_amount, TWO_ISA);
    assert_eq!(event.event_type, BridgeEventType::CreditRedemption);
    assert_eq!(event.direction, BridgeDirection::OffChainToOnChain);
    assert_eq!(event.status, BridgeEventStatus::Initiated);
}

#[test]
fn test_invalid_amount_purchase() {
    let mut bridge = setup();

    assert_eq!(
        bridge.initiate_purchase(user(), 0, HUNDRED_CREDITS, 1),
        Err(CreditBridgeError::InvalidAmount)
    );

    assert_eq!(
        bridge.initiate_purchase(user(), TWO_ISA, 0, 1),
        Err(CreditBridgeError::InvalidAmount)
    );
}

#[test]
fn test_invalid_amount_redemption() {
    let mut bridge = setup();

    assert_eq!(
        bridge.initiate_redemption(user(), 0, TWO_ISA, 1),
        Err(CreditBridgeError::InvalidAmount)
    );

    assert_eq!(
        bridge.initiate_redemption(user(), HUNDRED_CREDITS, 0, 1),
        Err(CreditBridgeError::InvalidAmount)
    );
}

#[test]
fn test_get_user_events() {
    let mut bridge = setup();

    bridge
        .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 1)
        .unwrap();
    bridge
        .initiate_redemption(user(), HUNDRED_CREDITS, TWO_ISA, 2)
        .unwrap();
    bridge
        .initiate_purchase(user2(), TWO_ISA, HUNDRED_CREDITS, 3)
        .unwrap();

    let user_events = bridge.get_user_events(&user());
    assert_eq!(user_events.len(), 2);
    assert_eq!(user_events[0].event_type, BridgeEventType::CreditPurchase);
    assert_eq!(user_events[1].event_type, BridgeEventType::CreditRedemption);

    let user2_events = bridge.get_user_events(&user2());
    assert_eq!(user2_events.len(), 1);

    // Unknown user returns empty
    assert!(bridge.get_user_events(&unknown_user()).is_empty());
}

#[test]
fn test_event_not_found() {
    let mut bridge = setup();
    let phantom = Hash::hash_data(b"nonexistent");

    assert_eq!(
        bridge.complete_event(&phantom),
        Err(CreditBridgeError::EventNotFound)
    );
    assert_eq!(
        bridge.fail_event(&phantom, "gone".to_string()),
        Err(CreditBridgeError::EventNotFound)
    );
    assert!(bridge.get_event(&phantom).is_none());
}
