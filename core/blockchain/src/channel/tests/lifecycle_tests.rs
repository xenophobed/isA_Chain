use super::*;
use crate::channel::types::{ChannelError, ChannelStatus, ChannelUpdate};

#[test]
fn test_open_channel() {
    let mut mgr = manager();
    let id = open_channel(&mut mgr);

    let ch = mgr.get_channel(&id).unwrap();
    assert_eq!(ch.sender, sender());
    assert_eq!(ch.receiver, receiver());
    assert_eq!(ch.deposit, DEPOSIT);
    assert_eq!(ch.sender_balance, DEPOSIT);
    assert_eq!(ch.receiver_balance, 0);
    assert_eq!(ch.nonce, 0);
    assert_eq!(ch.status, ChannelStatus::Open);
    assert_eq!(ch.opened_at, OPEN_HEIGHT);
    assert_eq!(ch.expires_at, OPEN_HEIGHT + DEFAULT_EXPIRY);
    assert_eq!(ch.dispute_period, DISPUTE_PERIOD);
}

#[test]
fn test_open_same_participants_fails() {
    let mut mgr = manager();
    let result = mgr.open_channel(sender(), sender(), DEPOSIT, OPEN_HEIGHT);
    assert_eq!(result, Err(ChannelError::SameParticipants));
}

#[test]
fn test_update_channel() {
    let mut mgr = manager();
    let id = open_channel(&mut mgr);

    let update = ChannelUpdate {
        channel_id: id,
        nonce: 1,
        sender_balance: 700,
        receiver_balance: 300,
    };
    mgr.update_channel(&id, update).unwrap();

    let ch = mgr.get_channel(&id).unwrap();
    assert_eq!(ch.nonce, 1);
    assert_eq!(ch.sender_balance, 700);
    assert_eq!(ch.receiver_balance, 300);
}

#[test]
fn test_update_invalid_nonce_fails() {
    let mut mgr = manager();
    let id = open_channel(&mut mgr);

    // First update: nonce 1 — OK
    let update1 = ChannelUpdate {
        channel_id: id,
        nonce: 1,
        sender_balance: 800,
        receiver_balance: 200,
    };
    mgr.update_channel(&id, update1).unwrap();

    // Second update with same nonce — should fail
    let update2 = ChannelUpdate {
        channel_id: id,
        nonce: 1,
        sender_balance: 600,
        receiver_balance: 400,
    };
    let result = mgr.update_channel(&id, update2);
    assert_eq!(result, Err(ChannelError::InvalidNonce));
}

#[test]
fn test_update_invalid_balances_fails() {
    let mut mgr = manager();
    let id = open_channel(&mut mgr);

    // sender_balance + receiver_balance != deposit (1_000)
    let update = ChannelUpdate {
        channel_id: id,
        nonce: 1,
        sender_balance: 500,
        receiver_balance: 400, // 500 + 400 = 900 ≠ 1_000
    };
    let result = mgr.update_channel(&id, update);
    assert_eq!(result, Err(ChannelError::InvalidBalances));
}

#[test]
fn test_close_channel() {
    let mut mgr = manager();
    let id = open_channel(&mut mgr);

    mgr.close_channel(&id, &sender(), 200).unwrap();

    let ch = mgr.get_channel(&id).unwrap();
    assert_eq!(ch.status, ChannelStatus::Closing { initiated_at: 200 });
}

#[test]
fn test_top_up() {
    let mut mgr = manager();
    let id = open_channel(&mut mgr);

    mgr.top_up(&id, 500).unwrap();

    let ch = mgr.get_channel(&id).unwrap();
    assert_eq!(ch.deposit, DEPOSIT + 500);
    assert_eq!(ch.sender_balance, DEPOSIT + 500);
    assert_eq!(ch.receiver_balance, 0);
}

#[test]
fn test_channel_expiry() {
    let mut mgr = manager();
    let id = open_channel(&mut mgr);

    // Not expired yet
    let still_open = mgr.check_expired(&id, OPEN_HEIGHT + DEFAULT_EXPIRY - 1).unwrap();
    assert!(!still_open);
    assert_eq!(mgr.get_channel(&id).unwrap().status, ChannelStatus::Open);

    // Exactly at expiry height — should expire
    let expired = mgr.check_expired(&id, OPEN_HEIGHT + DEFAULT_EXPIRY).unwrap();
    assert!(expired);
    assert_eq!(mgr.get_channel(&id).unwrap().status, ChannelStatus::Expired);
}

#[test]
fn test_not_participant_fails() {
    let mut mgr = manager();
    let id = open_channel(&mut mgr);

    let result = mgr.close_channel(&id, &third_party(), 200);
    assert_eq!(result, Err(ChannelError::NotParticipant(third_party())));

    let dispute_update = ChannelUpdate {
        channel_id: id,
        nonce: 1,
        sender_balance: 500,
        receiver_balance: 500,
    };
    let result2 = mgr.dispute_channel(&id, dispute_update, &third_party(), 200);
    assert_eq!(result2, Err(ChannelError::NotParticipant(third_party())));
}
