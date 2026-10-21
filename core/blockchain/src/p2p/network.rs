use super::types::*;
use crate::types::{BlockHeight, ChainId, Hash, Timestamp};
use std::collections::HashMap;

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
        let start = len.saturating_sub(count);
        self.message_log[start..].iter().collect()
    }
}
