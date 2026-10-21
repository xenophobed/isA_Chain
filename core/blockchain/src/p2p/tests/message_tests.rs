use super::helpers::*;
use crate::p2p::types::GossipMessage;
use crate::types::Hash;

#[test]
fn test_broadcast_message() {
    let mut net = default_network();
    let msg = GossipMessage::NewBlock(42, Hash::ZERO);
    net.broadcast_message(msg, 5000);

    assert_eq!(net.message_log.len(), 1);
    let recent = net.get_recent_messages(5);
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].1, 5000);
}

#[test]
fn test_get_recent_messages() {
    let mut net = default_network();

    for i in 0u64..10 {
        net.broadcast_message(GossipMessage::NewTransaction(Hash::ZERO), i * 100);
    }

    let recent = net.get_recent_messages(3);
    assert_eq!(recent.len(), 3);
    // Last 3 timestamps should be 700, 800, 900.
    assert_eq!(recent[0].1, 700);
    assert_eq!(recent[2].1, 900);
}
