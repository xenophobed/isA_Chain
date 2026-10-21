use super::helpers::*;
use crate::p2p::network::P2PNetwork;
use crate::p2p::types::{P2PError, PeerStatus};
use crate::types::Hash;

#[test]
fn test_connect_peer() {
    let mut net = default_network();
    let peer = make_peer(1, 1, 1000);
    assert!(net.connect_peer(peer).is_ok());
    assert_eq!(net.peer_count(), 1);
}

#[test]
fn test_max_peers() {
    let local = make_peer_id(0);
    let mut net = P2PNetwork::new(local, 1, 2, 1);

    net.connect_peer(make_peer(1, 1, 1000)).unwrap();
    net.connect_peer(make_peer(2, 1, 1000)).unwrap();

    let result = net.connect_peer(make_peer(3, 1, 1000));
    assert_eq!(result, Err(P2PError::MaxPeersReached));
}

#[test]
fn test_disconnect() {
    let mut net = default_network();
    let peer = make_peer(1, 1, 1000);
    let id = peer.peer_id.id;
    net.connect_peer(peer).unwrap();

    assert!(net.disconnect_peer(&id).is_ok());
    assert_eq!(
        net.get_peer(&id).unwrap().status,
        PeerStatus::Disconnected
    );
}

#[test]
fn test_ban_peer() {
    let mut net = default_network();
    let peer = make_peer(1, 1, 1000);
    let id = peer.peer_id.id;
    net.connect_peer(peer).unwrap();

    net.ban_peer(&id, 60_000, "spam".into(), 1000).unwrap();

    assert!(net.is_banned(&id, 1000));
    assert!(net.is_banned(&id, 60_999));

    let stored = net.get_peer(&id).unwrap();
    match &stored.status {
        PeerStatus::Banned { reason, .. } => assert_eq!(reason, "spam"),
        other => panic!("expected Banned, got {other:?}"),
    }
}

#[test]
fn test_ban_expires() {
    let mut net = default_network();
    let peer = make_peer(1, 1, 1000);
    let id = peer.peer_id.id;
    net.connect_peer(peer).unwrap();

    // Ban for 1 second.
    net.ban_peer(&id, 1_000, "test".into(), 1000).unwrap();

    // At ban_until the peer is still banned (now <= ban_until).
    assert!(net.is_banned(&id, 2_000));

    // After ban_until the peer is free.
    assert!(!net.is_banned(&id, 2_001));
}

#[test]
fn test_incompatible_chain() {
    let mut net = default_network(); // chain_id = 1
    let peer = make_peer(1, 99, 1000); // wrong chain

    let result = net.connect_peer(peer);
    assert_eq!(result, Err(P2PError::IncompatibleChain));
}

#[test]
fn test_get_best_peer() {
    let mut net = default_network();

    let mut p1 = make_peer(1, 1, 1000);
    p1.best_height = 100;
    let id1 = p1.peer_id.id;

    let mut p2 = make_peer(2, 1, 1000);
    p2.best_height = 200;

    net.connect_peer(p1).unwrap();
    net.connect_peer(p2).unwrap();

    let best = net.get_best_peer().unwrap();
    assert_eq!(best.best_height, 200);
    assert_ne!(best.peer_id.id, id1);
}

#[test]
fn test_needs_peers() {
    let local = make_peer_id(0);
    let mut net = P2PNetwork::new(local, 1, 50, 3);

    // Below min_peers (3).
    assert!(net.needs_peers());

    net.connect_peer(make_peer(1, 1, 1000)).unwrap();
    net.connect_peer(make_peer(2, 1, 1000)).unwrap();
    net.connect_peer(make_peer(3, 1, 1000)).unwrap();

    // At min_peers, no longer needs peers.
    assert!(!net.needs_peers());
}

#[test]
fn test_update_height() {
    let mut net = default_network();
    let peer = make_peer(1, 1, 1000);
    let id = peer.peer_id.id;
    net.connect_peer(peer).unwrap();

    let new_hash = Hash::hash_data(b"block500");
    net.update_peer_height(&id, 500, new_hash).unwrap();

    let stored = net.get_peer(&id).unwrap();
    assert_eq!(stored.best_height, 500);
    assert_eq!(stored.best_hash, new_hash);
}

#[test]
fn test_already_connected() {
    let mut net = default_network();
    let peer = make_peer(1, 1, 1000);
    net.connect_peer(peer.clone()).unwrap();

    let result = net.connect_peer(peer);
    assert_eq!(result, Err(P2PError::AlreadyConnected));
}

#[test]
fn test_get_connected_peers() {
    let mut net = default_network();

    let p1 = make_peer(1, 1, 1000);
    let p2 = make_peer(2, 1, 1000);
    let id2 = p2.peer_id.id;

    net.connect_peer(p1).unwrap();
    net.connect_peer(p2).unwrap();

    // Disconnect peer 2.
    net.disconnect_peer(&id2).unwrap();

    let connected = net.get_connected_peers();
    assert_eq!(connected.len(), 1);
    assert_eq!(connected[0].peer_id.id, Hash::new([1u8; 32]));
}

#[test]
fn test_peer_not_found_disconnect() {
    let mut net = default_network();
    let fake_id = Hash::new([42u8; 32]);

    let result = net.disconnect_peer(&fake_id);
    assert_eq!(result, Err(P2PError::PeerNotFound(fake_id)));
}

#[test]
fn test_peer_not_found_update_height() {
    let mut net = default_network();
    let fake_id = Hash::new([99u8; 32]);

    let result = net.update_peer_height(&fake_id, 1, Hash::ZERO);
    assert_eq!(result, Err(P2PError::PeerNotFound(fake_id)));
}
