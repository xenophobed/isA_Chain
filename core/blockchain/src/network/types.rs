use crate::block::Block;
use crate::transaction::Transaction;
use crate::types::*;
use serde::{Deserialize, Serialize};

// ============================================================================
// Network Protocol Messages
// ============================================================================

/// Network protocol messages exchanged between nodes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetworkMessage {
    /// Handshake on connection.
    Hello {
        version: u32,
        chain_id: ChainId,
        best_height: BlockHeight,
        best_hash: Hash,
        genesis_hash: Hash,
    },
    /// Request blocks by height range.
    GetBlocks {
        start_height: BlockHeight,
        count: u32,
    },
    /// Response with a batch of blocks.
    Blocks(Vec<Block>),
    /// Announce a newly minted block.
    NewBlock {
        height: BlockHeight,
        hash: Hash,
    },
    /// Announce a newly seen transaction.
    NewTransaction(Hash),
    /// Request a single block by hash.
    GetBlock(Hash),
    /// Request specific transactions by hash.
    GetTransactions(Vec<Hash>),
    /// Transactions response.
    Transactions(Vec<Transaction>),
    /// Request the remote peer's status.
    Status,
    /// Response to a Status request.
    StatusResponse {
        height: BlockHeight,
        head: Hash,
        peer_count: usize,
    },
    /// Keep-alive ping carrying a nonce.
    Ping(u64),
    /// Keep-alive pong echoing the ping nonce.
    Pong(u64),
}

// ============================================================================
// Network Configuration
// ============================================================================

/// Runtime configuration for the network layer.
#[derive(Clone, Debug)]
pub struct NetworkConfig {
    /// TCP port the node listens on.
    pub listen_port: u16,
    /// Maximum number of simultaneous peer connections.
    pub max_peers: usize,
    /// Seed nodes used for initial peer discovery (multiaddr strings).
    pub bootstrap_nodes: Vec<String>,
    /// Chain ID this node belongs to.
    pub chain_id: ChainId,
    /// Numeric protocol version advertised during handshake.
    pub protocol_version: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_port: 30333,
            max_peers: 50,
            bootstrap_nodes: Vec::new(),
            chain_id: 1,
            protocol_version: 1,
        }
    }
}

// ============================================================================
// Sync Status
// ============================================================================

/// Synchronisation state of this node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncStatus {
    /// No sync in progress.
    Idle,
    /// Actively downloading blocks towards `target_height`.
    Syncing {
        target_height: BlockHeight,
        current_height: BlockHeight,
    },
    /// Local chain is at parity with the best known peer.
    Synced,
}
