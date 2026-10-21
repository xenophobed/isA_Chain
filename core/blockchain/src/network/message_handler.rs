use crate::error::NetworkError;
use crate::types::{BlockHeight, ChainId, Hash};

use super::types::{NetworkConfig, NetworkMessage};

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
