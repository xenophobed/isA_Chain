use crate::types::{Address, BlockHeight, ChainId, Hash, Timestamp};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
