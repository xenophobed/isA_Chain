use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::{Address, BlockHeight, ChainId, Hash, Timestamp};

// ============================================================================
// Types
// ============================================================================

/// Unique identifier for a peer, derived from its public key.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId {
    /// Hash derived from the peer's public key.
    pub id: Hash,
    /// Multiaddr-style address string (e.g. "/ip4/127.0.0.1/tcp/30333").
    pub address: String,
}

impl PeerId {
    pub fn new(id: Hash, address: impl Into<String>) -> Self {
        Self {
            id,
            address: address.into(),
        }
    }
}

/// Runtime information about a connected or known peer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub version: String,
    pub chain_id: ChainId,
    pub best_height: BlockHeight,
    pub best_hash: Hash,
    pub connected_at: Timestamp,
    pub last_seen: Timestamp,
    pub latency_ms: u64,
    pub status: PeerStatus,
}

impl PeerInfo {
    pub fn new(
        peer_id: PeerId,
        version: impl Into<String>,
        chain_id: ChainId,
        now: Timestamp,
    ) -> Self {
        Self {
            peer_id,
            version: version.into(),
            chain_id,
            best_height: 0,
            best_hash: Hash::ZERO,
            connected_at: now,
            last_seen: now,
            latency_ms: 0,
            status: PeerStatus::Connected,
        }
    }
}

/// Connection / ban state of a peer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerStatus {
    Connected,
    Disconnected,
    Banned { until: Timestamp, reason: String },
    Syncing,
}

/// Messages propagated over the gossip layer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GossipMessage {
    /// Announcement of a new block.
    NewBlock(BlockHeight, Hash),
    /// Announcement of a new transaction.
    NewTransaction(Hash),
    /// Consensus vote for a specific block.
    ConsensusVote {
        height: BlockHeight,
        voter: Address,
        block_hash: Hash,
    },
    /// Peer list advertisement for peer discovery.
    PeerDiscovery(Vec<PeerId>),
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, Error, PartialEq, Eq)]
pub enum P2PError {
    #[error("max peers reached")]
    MaxPeersReached,

    #[error("peer not found: {0:?}")]
    PeerNotFound(Hash),

    #[error("peer is banned")]
    PeerBanned,

    #[error("incompatible chain id")]
    IncompatibleChain,

    #[error("incompatible protocol version")]
    IncompatibleVersion,

    #[error("peer already connected")]
    AlreadyConnected,
}

// ============================================================================
// P2PNetwork
// ============================================================================

/// Peer-to-peer network manager. Handles peer lifecycle, gossip logging,
/// and basic peer-selection heuristics for multi-validator testnets.
pub struct P2PNetwork {
    /// Active and known peers, keyed by peer_id.id.
    pub peers: HashMap<Hash, PeerInfo>,
    /// Maximum number of concurrent peers.
    pub max_peers: usize,
    /// Minimum number of peers before the node seeks more connections.
    pub min_peers: usize,
    /// Chain ID this node belongs to.
    pub chain_id: ChainId,
    /// This node's own peer identity.
    pub local_peer_id: PeerId,
    /// In-memory log of recently broadcast gossip messages.
    pub message_log: Vec<(GossipMessage, Timestamp)>,
    /// Peers that are temporarily banned: peer_id.id -> ban_expiry timestamp.
    pub banned_peers: HashMap<Hash, Timestamp>,
    /// Protocol version string advertised to peers.
    pub version: String,
}

impl P2PNetwork {
    // ------------------------------------------------------------------
    // Construction
    // ------------------------------------------------------------------

    pub fn new(
        local_peer_id: PeerId,
        chain_id: ChainId,
        max_peers: usize,
        min_peers: usize,
    ) -> Self {
        Self {
            peers: HashMap::new(),
            max_peers,
            min_peers,
            chain_id,
            local_peer_id,
            message_log: Vec::new(),
            banned_peers: HashMap::new(),
            version: String::from("1.0.0"),
        }
    }

    // ------------------------------------------------------------------
    // Peer management
    // ------------------------------------------------------------------

    /// Attempt to add a peer. Validates chain compatibility, peer count
    /// limits, ban status, and duplicate connections.
    pub fn connect_peer(&mut self, peer: PeerInfo) -> Result<(), P2PError> {
        let id = peer.peer_id.id;

        // Chain ID must match.
        if peer.chain_id != self.chain_id {
            return Err(P2PError::IncompatibleChain);
        }

        // Reject banned peers (check against current time embedded in peer.connected_at
        // as the "now" the caller passed). We use the ban map directly.
        if self.banned_peers.contains_key(&id) {
            // The caller doesn't pass `now` here, but we can check the stored
            // expiry: if any ban entry exists and isn't obviously expired we
            // reject. Because the caller controls `connected_at` we use it as
            // the current time for the expiry check.
            let ban_until = self.banned_peers[&id];
            if peer.connected_at <= ban_until {
                return Err(P2PError::PeerBanned);
            }
            // Ban has expired — clean it up and proceed.
            self.banned_peers.remove(&id);
        }

        // Reject duplicates.
        if self.peers.contains_key(&id) {
            return Err(P2PError::AlreadyConnected);
        }

        // Enforce peer cap.
        if self.peers.len() >= self.max_peers {
            return Err(P2PError::MaxPeersReached);
        }

        self.peers.insert(id, peer);
        Ok(())
    }

    /// Remove a peer from the active set, marking it as disconnected.
    pub fn disconnect_peer(&mut self, peer_id: &Hash) -> Result<(), P2PError> {
        let peer = self
            .peers
            .get_mut(peer_id)
            .ok_or(P2PError::PeerNotFound(*peer_id))?;
        peer.status = PeerStatus::Disconnected;
        Ok(())
    }

    /// Ban a peer for `duration_ms` milliseconds starting from `now`.
    pub fn ban_peer(
        &mut self,
        peer_id: &Hash,
        duration_ms: u64,
        reason: String,
        now: Timestamp,
    ) -> Result<(), P2PError> {
        let ban_until = now + duration_ms;

        // Update ban map.
        self.banned_peers.insert(*peer_id, ban_until);

        // Update peer record if present.
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.status = PeerStatus::Banned {
                until: ban_until,
                reason,
            };
        }

        Ok(())
    }

    /// Returns `true` if the peer is currently under a ban that has not expired.
    pub fn is_banned(&self, peer_id: &Hash, now: Timestamp) -> bool {
        match self.banned_peers.get(peer_id) {
            Some(&ban_until) => now <= ban_until,
            None => false,
        }
    }

    /// Look up a peer by its id hash.
    pub fn get_peer(&self, peer_id: &Hash) -> Option<&PeerInfo> {
        self.peers.get(peer_id)
    }

    /// Return all peers whose status is `Connected` or `Syncing`.
    pub fn get_connected_peers(&self) -> Vec<&PeerInfo> {
        self.peers
            .values()
            .filter(|p| {
                matches!(p.status, PeerStatus::Connected | PeerStatus::Syncing)
            })
            .collect()
    }

    /// Append a gossip message to the in-memory log.
    pub fn broadcast_message(&mut self, msg: GossipMessage, timestamp: Timestamp) {
        self.message_log.push((msg, timestamp));
    }

    /// Return the connected peer with the highest known block height.
    pub fn get_best_peer(&self) -> Option<&PeerInfo> {
        self.get_connected_peers()
            .into_iter()
            .max_by_key(|p| p.best_height)
    }

    /// Total number of tracked peers (all statuses).
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// `true` when the number of *connected* peers is below `min_peers`.
    pub fn needs_peers(&self) -> bool {
        self.get_connected_peers().len() < self.min_peers
    }

    /// Update the best-known chain tip for a peer.
    pub fn update_peer_height(
        &mut self,
        peer_id: &Hash,
        height: BlockHeight,
        hash: Hash,
    ) -> Result<(), P2PError> {
        let peer = self
            .peers
            .get_mut(peer_id)
            .ok_or(P2PError::PeerNotFound(*peer_id))?;
        peer.best_height = height;
        peer.best_hash = hash;
        Ok(())
    }

    /// Return the `count` most-recent messages from the log.
    pub fn get_recent_messages(&self, count: usize) -> Vec<&(GossipMessage, Timestamp)> {
        let len = self.message_log.len();
        let start = if len > count { len - count } else { 0 };
        self.message_log[start..].iter().collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn make_peer_id(seed: u8) -> PeerId {
        PeerId::new(
            Hash::new([seed; 32]),
            format!("/ip4/127.0.0.{seed}/tcp/30333"),
        )
    }

    fn make_peer(seed: u8, chain_id: ChainId, now: Timestamp) -> PeerInfo {
        PeerInfo::new(make_peer_id(seed), "1.0.0", chain_id, now)
    }

    fn default_network() -> P2PNetwork {
        let local = make_peer_id(0);
        P2PNetwork::new(local, 1, 50, 3)
    }

    // ------------------------------------------------------------------
    // Tests
    // ------------------------------------------------------------------

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
}
