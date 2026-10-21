use super::*;
use crate::channel::types::{ChannelError, ChannelStatus, ChannelUpdate};

#[test]
fn test_finalize_after_dispute_period() {
    let mut mgr = manager();
    let id = open_channel(&mut mgr);

    // Partial payment: 300 to receiver
    let update = ChannelUpdate {
        channel_id: id,
        nonce: 1,
        sender_balance: 700,
        receiver_balance: 300,
    };
    mgr.update_channel(&id, update).unwrap();

    // Initiate close at block 200
    mgr.close_channel(&id, &sender(), 200).unwrap();

    // Finalise after dispute period (200 + 50 = 250)
    let (sender_refund, receiver_payout) = mgr.finalize_channel(&id, 250).unwrap();

    assert_eq!(sender_refund, 700);
    assert_eq!(receiver_payout, 300);
    assert_eq!(mgr.get_channel(&id).unwrap().status, ChannelStatus::Closed);
}

#[test]
fn test_finalize_before_period_fails() {
    let mut mgr = manager();
    let id = open_channel(&mut mgr);

    mgr.close_channel(&id, &sender(), 200).unwrap();

    // Try to finalise one block too early (200 + 50 - 1 = 249)
    let result = mgr.finalize_channel(&id, 249);
    assert_eq!(result, Err(ChannelError::DisputePeriodNotOver));
}

#[test]
fn test_dispute_channel() {
    let mut mgr = manager();
    let id = open_channel(&mut mgr);

    // Apply update nonce 1
    let update1 = ChannelUpdate {
        channel_id: id,
        nonce: 1,
        sender_balance: 600,
        receiver_balance: 400,
    };
    mgr.update_channel(&id, update1).unwrap();

    // Sender tries to close at nonce 1 (stale state favourable to them)
    mgr.close_channel(&id, &sender(), 200).unwrap();

    // Receiver disputes with a higher nonce
    let dispute_update = ChannelUpdate {
        channel_id: id,
        nonce: 2,
        sender_balance: 200,
        receiver_balance: 800,
    };
    mgr.dispute_channel(&id, dispute_update, &receiver(), 210)
        .unwrap();

    let ch = mgr.get_channel(&id).unwrap();
    assert_eq!(ch.status, ChannelStatus::Disputed { disputed_at: 210 });
    assert_eq!(ch.nonce, 2);
    assert_eq!(ch.sender_balance, 200);
    assert_eq!(ch.receiver_balance, 800);

    // Finalise after dispute period
    let (refund, payout) = mgr.finalize_channel(&id, 260).unwrap();
    assert_eq!(refund, 200);
    assert_eq!(payout, 800);
}

#[test]
fn test_get_sender_receiver_channels() {
    let mut mgr = manager();

    // Open two channels: sender → receiver (heights 100 and 200 to get distinct IDs)
    let id1 = mgr
        .open_channel(sender(), receiver(), 1_000, 100)
        .unwrap();
    let id2 = mgr
        .open_channel(sender(), receiver(), 500, 200)
        .unwrap();

    let sender_chans = mgr.get_sender_channels(&sender());
    assert_eq!(sender_chans.len(), 2);
    let ids: Vec<Hash> = sender_chans.iter().map(|c| c.id).collect();
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id2));

    let receiver_chans = mgr.get_receiver_channels(&receiver());
    assert_eq!(receiver_chans.len(), 2);

    // Third party has no channels
    assert!(mgr.get_sender_channels(&third_party()).is_empty());
    assert!(mgr.get_receiver_channels(&third_party()).is_empty());

    // Active count: 2 open channels
    assert_eq!(mgr.active_channel_count(), 2);

    // Close one and verify count drops
    mgr.close_channel(&id1, &sender(), 300).unwrap();
    assert_eq!(mgr.active_channel_count(), 1);
}
