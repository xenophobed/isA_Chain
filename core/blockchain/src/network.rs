use crate::block::Block;
use crate::error::NetworkError;
use crate::transaction::Transaction;
use crate::types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

// ============================================================================
// Sync Manager
// ============================================================================

/// Determines which blocks to request and tracks in-flight requests.
pub struct SyncManager {
    /// Our current confirmed chain height.
    local_height: BlockHeight,
    /// Highest height advertised by any peer.
    best_peer_height: BlockHeight,
    /// Current sync status.
    status: SyncStatus,
    /// Blocks that have been requested but not yet received:
    /// maps start_height → time of request.
    pending_requests: HashMap<BlockHeight, std::time::Instant>,
    /// Maximum number of outstanding block requests.
    max_concurrent_requests: usize,
}

impl SyncManager {
    /// Create a new `SyncManager` starting from `local_height`.
    pub fn new(local_height: BlockHeight) -> Self {
        Self {
            local_height,
            best_peer_height: local_height,
            // When no peers are known yet, local == best → Synced.
            status: SyncStatus::Synced,
            pending_requests: HashMap::new(),
            max_concurrent_requests: 16,
        }
    }

    /// Notify the manager that a peer is at `height`.
    pub fn update_peer_height(&mut self, height: BlockHeight) {
        if height > self.best_peer_height {
            self.best_peer_height = height;
        }
        self.refresh_status();
    }

    /// Current sync status.
    pub fn get_sync_status(&self) -> &SyncStatus {
        &self.status
    }

    /// `true` when our chain is behind the best known peer.
    pub fn needs_sync(&self) -> bool {
        self.local_height < self.best_peer_height
    }

    /// Returns the next `(start_height, count)` range to request, or `None` if
    /// sync is not needed or the concurrent-request limit has been reached.
    pub fn get_blocks_to_request(&mut self, batch_size: u32) -> Option<(BlockHeight, u32)> {
        if !self.needs_sync() {
            return None;
        }
        if self.pending_requests.len() >= self.max_concurrent_requests {
            return None;
        }

        // Find the lowest height above local_height that isn't already pending.
        let next = self.local_height + 1;

        // Walk forward until we find a slot not in pending_requests.
        let mut candidate = next;
        while self.pending_requests.contains_key(&candidate) {
            candidate += batch_size as BlockHeight;
        }

        if candidate > self.best_peer_height {
            return None;
        }

        // Clamp the count so we don't request past best_peer_height.
        let remaining = self.best_peer_height - candidate + 1;
        let count = (batch_size as u64).min(remaining) as u32;

        self.pending_requests
            .insert(candidate, std::time::Instant::now());

        Some((candidate, count))
    }

    /// Record that the block at `height` has been received and applied.
    pub fn mark_received(&mut self, height: BlockHeight) {
        self.pending_requests.remove(&height);
        if height > self.local_height {
            self.local_height = height;
        }
        self.refresh_status();
    }

    /// Explicitly update local chain height (e.g. after block import).
    pub fn update_local_height(&mut self, height: BlockHeight) {
        self.local_height = height;
        self.refresh_status();
    }

    /// `true` when the local chain is at or ahead of the best peer.
    pub fn is_synced(&self) -> bool {
        self.local_height >= self.best_peer_height
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    fn refresh_status(&mut self) {
        self.status = if self.local_height >= self.best_peer_height {
            SyncStatus::Synced
        } else {
            SyncStatus::Syncing {
                target_height: self.best_peer_height,
                current_height: self.local_height,
            }
        };
    }
}

// ============================================================================
// Message Handler
// ============================================================================

/// Constructs and validates protocol-level messages.
pub struct MessageHandler {
    chain_id: ChainId,
    genesis_hash: Hash,
    protocol_version: u32,
}

impl MessageHandler {
    /// Create a new handler for the given chain.
    pub fn new(chain_id: ChainId, genesis_hash: Hash) -> Self {
        Self {
            chain_id,
            genesis_hash,
            protocol_version: 1,
        }
    }

    /// Build a `Hello` message advertising our current chain tip.
    pub fn create_hello(&self, height: BlockHeight, head: Hash) -> NetworkMessage {
        NetworkMessage::Hello {
            version: self.protocol_version,
            chain_id: self.chain_id,
            best_height: height,
            best_hash: head,
            genesis_hash: self.genesis_hash,
        }
    }

    /// Validate an incoming `Hello` message.
    ///
    /// Returns `Ok(())` when the handshake is compatible, or a `NetworkError`
    /// describing the specific incompatibility.
    pub fn validate_hello(&self, msg: &NetworkMessage) -> Result<(), NetworkError> {
        match msg {
            NetworkMessage::Hello {
                version,
                chain_id,
                genesis_hash,
                ..
            } => {
                if *chain_id != self.chain_id {
                    return Err(NetworkError::InvalidMessage {
                        peer_id: format!(
                            "chain_id mismatch: expected {}, got {}",
                            self.chain_id, chain_id
                        ),
                    });
                }
                if *version != self.protocol_version {
                    return Err(NetworkError::ProtocolMismatch {
                        expected: self.protocol_version,
                        actual: *version,
                    });
                }
                if *genesis_hash != self.genesis_hash {
                    return Err(NetworkError::InvalidMessage {
                        peer_id: format!(
                            "genesis hash mismatch: expected {:?}, got {:?}",
                            self.genesis_hash, genesis_hash
                        ),
                    });
                }
                Ok(())
            }
            _ => Err(NetworkError::InvalidMessage {
                peer_id: "expected Hello message".to_string(),
            }),
        }
    }

    /// Build a `StatusResponse` message.
    pub fn create_status(
        &self,
        height: BlockHeight,
        head: Hash,
        peers: usize,
    ) -> NetworkMessage {
        NetworkMessage::StatusResponse {
            height,
            head,
            peer_count: peers,
        }
    }
}

// ============================================================================
// Legacy NetworkManager (kept for backwards compatibility)
// ============================================================================

/// Thin wrapper retained so that existing call-sites continue to compile.
/// New code should use `SyncManager` and `MessageHandler` directly.
pub struct NetworkManager {
    /// Local peer ID string.
    pub peer_id: String,
    /// Connected peer addresses.
    pub peers: Vec<String>,
    /// Network configuration.
    pub config: NetworkConfig,
}

impl NetworkManager {
    pub fn new(config: NetworkConfig) -> Self {
        NetworkManager {
            peer_id: format!("peer_{}", config.listen_port),
            peers: Vec::new(),
            config,
        }
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

    fn genesis() -> Hash {
        Hash::hash_data(b"genesis")
    }

    fn head() -> Hash {
        Hash::hash_data(b"head_block")
    }

    // ------------------------------------------------------------------
    // NetworkMessage serialisation
    // ------------------------------------------------------------------

    /// Round-trip every variant through bincode to verify Serialize/Deserialize
    /// are correctly derived.
    #[test]
    fn test_network_message_serialization() {
        let messages: Vec<NetworkMessage> = vec![
            NetworkMessage::Hello {
                version: 1,
                chain_id: 42,
                best_height: 100,
                best_hash: head(),
                genesis_hash: genesis(),
            },
            NetworkMessage::GetBlocks {
                start_height: 50,
                count: 10,
            },
            NetworkMessage::Blocks(vec![]),
            NetworkMessage::NewBlock {
                height: 101,
                hash: head(),
            },
            NetworkMessage::NewTransaction(Hash::ZERO),
            NetworkMessage::GetBlock(Hash::ZERO),
            NetworkMessage::GetTransactions(vec![Hash::ZERO, head()]),
            NetworkMessage::Transactions(vec![]),
            NetworkMessage::Status,
            NetworkMessage::StatusResponse {
                height: 100,
                head: head(),
                peer_count: 5,
            },
            NetworkMessage::Ping(12345),
            NetworkMessage::Pong(12345),
        ];

        for msg in &messages {
            let encoded = bincode::serialize(msg).expect("serialize failed");
            let decoded: NetworkMessage =
                bincode::deserialize(&encoded).expect("deserialize failed");

            // Verify the discriminant / key fields survive the round-trip.
            match (msg, &decoded) {
                (NetworkMessage::Hello { version: v1, .. }, NetworkMessage::Hello { version: v2, .. }) => {
                    assert_eq!(v1, v2)
                }
                (NetworkMessage::GetBlocks { count: c1, .. }, NetworkMessage::GetBlocks { count: c2, .. }) => {
                    assert_eq!(c1, c2)
                }
                (NetworkMessage::Ping(n1), NetworkMessage::Pong(_)) => {
                    panic!("Ping decoded as Pong: nonce={n1}")
                }
                (NetworkMessage::Ping(n1), NetworkMessage::Ping(n2)) => assert_eq!(n1, n2),
                (NetworkMessage::Pong(n1), NetworkMessage::Pong(n2)) => assert_eq!(n1, n2),
                (NetworkMessage::StatusResponse { height: h1, .. }, NetworkMessage::StatusResponse { height: h2, .. }) => {
                    assert_eq!(h1, h2)
                }
                _ => {
                    // For unit variants (Status, Blocks(empty), …) just confirm
                    // no panic — the encode/decode succeeded above.
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // SyncManager
    // ------------------------------------------------------------------

    #[test]
    fn test_sync_manager_needs_sync() {
        let mut mgr = SyncManager::new(10);
        assert!(!mgr.needs_sync(), "no peers yet, should not need sync");

        mgr.update_peer_height(20);
        assert!(mgr.needs_sync());
    }

    #[test]
    fn test_sync_manager_get_blocks_to_request() {
        let mut mgr = SyncManager::new(0);
        mgr.update_peer_height(100);

        let req = mgr.get_blocks_to_request(32);
        assert!(req.is_some());
        let (start, count) = req.unwrap();
        assert_eq!(start, 1, "should request from height 1");
        assert_eq!(count, 32);
    }

    #[test]
    fn test_sync_manager_get_blocks_no_request_when_synced() {
        let mut mgr = SyncManager::new(50);
        mgr.update_peer_height(50);

        let req = mgr.get_blocks_to_request(16);
        assert!(req.is_none(), "already synced, nothing to request");
    }

    #[test]
    fn test_sync_manager_mark_received() {
        let mut mgr = SyncManager::new(0);
        mgr.update_peer_height(5);

        // Request a batch and then mark each block received.
        mgr.get_blocks_to_request(5).unwrap();
        mgr.mark_received(1);
        mgr.mark_received(2);
        mgr.mark_received(3);
        mgr.mark_received(4);
        mgr.mark_received(5);

        assert_eq!(mgr.local_height, 5);
    }

    #[test]
    fn test_sync_manager_synced() {
        let mut mgr = SyncManager::new(0);
        mgr.update_peer_height(3);

        assert!(!mgr.is_synced());

        mgr.update_local_height(3);
        assert!(mgr.is_synced());
        assert_eq!(*mgr.get_sync_status(), SyncStatus::Synced);
    }

    #[test]
    fn test_sync_manager_status_transitions() {
        let mut mgr = SyncManager::new(10);
        // Before any peers: local == best → Synced.
        assert_eq!(*mgr.get_sync_status(), SyncStatus::Synced);

        mgr.update_peer_height(20);
        assert_eq!(
            *mgr.get_sync_status(),
            SyncStatus::Syncing {
                target_height: 20,
                current_height: 10,
            }
        );

        mgr.update_local_height(20);
        assert_eq!(*mgr.get_sync_status(), SyncStatus::Synced);
    }

    // ------------------------------------------------------------------
    // MessageHandler
    // ------------------------------------------------------------------

    #[test]
    fn test_message_handler_hello() {
        let handler = MessageHandler::new(1, genesis());
        let msg = handler.create_hello(42, head());

        match msg {
            NetworkMessage::Hello {
                version,
                chain_id,
                best_height,
                best_hash,
                genesis_hash,
            } => {
                assert_eq!(version, 1);
                assert_eq!(chain_id, 1);
                assert_eq!(best_height, 42);
                assert_eq!(best_hash, head());
                assert_eq!(genesis_hash, genesis());
            }
            _ => panic!("expected Hello"),
        }
    }

    #[test]
    fn test_message_handler_validate_hello_ok() {
        let handler = MessageHandler::new(1, genesis());
        let hello = handler.create_hello(0, Hash::ZERO);
        assert!(handler.validate_hello(&hello).is_ok());
    }

    #[test]
    fn test_message_handler_validate_hello_wrong_chain() {
        let handler = MessageHandler::new(1, genesis());
        let wrong_chain = NetworkMessage::Hello {
            version: 1,
            chain_id: 99,
            best_height: 0,
            best_hash: Hash::ZERO,
            genesis_hash: genesis(),
        };
        let result = handler.validate_hello(&wrong_chain);
        assert!(result.is_err());
        matches!(result.unwrap_err(), NetworkError::InvalidMessage { .. });
    }

    #[test]
    fn test_message_handler_validate_hello_wrong_version() {
        let handler = MessageHandler::new(1, genesis());
        let wrong_version = NetworkMessage::Hello {
            version: 99,
            chain_id: 1,
            best_height: 0,
            best_hash: Hash::ZERO,
            genesis_hash: genesis(),
        };
        let result = handler.validate_hello(&wrong_version);
        assert!(result.is_err());
        matches!(
            result.unwrap_err(),
            NetworkError::ProtocolMismatch { expected: 1, actual: 99 }
        );
    }

    #[test]
    fn test_message_handler_validate_hello_wrong_genesis() {
        let handler = MessageHandler::new(1, genesis());
        let wrong_genesis = NetworkMessage::Hello {
            version: 1,
            chain_id: 1,
            best_height: 0,
            best_hash: Hash::ZERO,
            genesis_hash: Hash::ZERO, // wrong
        };
        let result = handler.validate_hello(&wrong_genesis);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_handler_validate_non_hello() {
        let handler = MessageHandler::new(1, genesis());
        let result = handler.validate_hello(&NetworkMessage::Ping(0));
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // NetworkConfig default
    // ------------------------------------------------------------------

    #[test]
    fn test_network_config_default() {
        let cfg = NetworkConfig::default();
        assert_eq!(cfg.listen_port, 30333);
        assert_eq!(cfg.max_peers, 50);
        assert!(cfg.bootstrap_nodes.is_empty());
        assert_eq!(cfg.chain_id, 1);
        assert_eq!(cfg.protocol_version, 1);
    }
}
