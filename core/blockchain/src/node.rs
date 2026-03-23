use std::sync::Arc;
use tokio::sync::RwLock;
use serde::Serialize;

use crate::blockchain::Blockchain;
use crate::types::ChainId;

// ---------------------------------------------------------------------------
// NodeConfig
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub chain_id: ChainId,
    pub rpc_port: u16,
    pub block_time_secs: u64,
    pub data_dir: String,
    pub persist: bool,
    pub max_txs_per_block: usize,
    pub produce_empty_blocks: bool,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            chain_id: crate::types::constants::MAIN_CHAIN_ID,
            rpc_port: 9944,
            block_time_secs: 3,
            data_dir: "./data".to_string(),
            persist: false,
            max_txs_per_block: 100,
            produce_empty_blocks: false,
        }
    }
}

impl NodeConfig {

    /// Read configuration from environment variables, falling back to defaults.
    ///
    /// Environment variables (all optional):
    /// - `CHAIN_ID`            → chain_id (u64)
    /// - `RPC_PORT`            → rpc_port (u16)
    /// - `BLOCK_TIME_SECS`     → block_time_secs (u64)
    /// - `DATA_DIR`            → data_dir (String)
    /// - `PERSIST`             → persist ("true"/"1" → true)
    /// - `MAX_TXS_PER_BLOCK`   → max_txs_per_block (usize)
    /// - `PRODUCE_EMPTY_BLOCKS`→ produce_empty_blocks ("true"/"1" → true)
    pub fn from_env() -> Self {
        let defaults = Self::default();

        let chain_id = std::env::var("CHAIN_ID")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.chain_id);

        let rpc_port = std::env::var("RPC_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.rpc_port);

        let block_time_secs = std::env::var("BLOCK_TIME_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.block_time_secs);

        let data_dir = std::env::var("DATA_DIR").unwrap_or(defaults.data_dir);

        let persist = std::env::var("PERSIST")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(defaults.persist);

        let max_txs_per_block = std::env::var("MAX_TXS_PER_BLOCK")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.max_txs_per_block);

        let produce_empty_blocks = std::env::var("PRODUCE_EMPTY_BLOCKS")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(defaults.produce_empty_blocks);

        Self {
            chain_id,
            rpc_port,
            block_time_secs,
            data_dir,
            persist,
            max_txs_per_block,
            produce_empty_blocks,
        }
    }
}

// ---------------------------------------------------------------------------
// NodeStatus
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum NodeStatus {
    Starting,
    Running,
    ShuttingDown,
    Stopped,
}

impl std::fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            NodeStatus::Starting => "starting",
            NodeStatus::Running => "running",
            NodeStatus::ShuttingDown => "shutting_down",
            NodeStatus::Stopped => "stopped",
        };
        write!(f, "{}", s)
    }
}

// ---------------------------------------------------------------------------
// NodeInfo
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct NodeInfo {
    pub chain_id: ChainId,
    pub rpc_port: u16,
    pub uptime_secs: u64,
    pub status: String,
    pub version: String,
    pub persist: bool,
}

// ---------------------------------------------------------------------------
// Node
// ---------------------------------------------------------------------------

pub struct Node {
    pub config: NodeConfig,
    pub status: NodeStatus,
    pub start_time: std::time::Instant,
    pub blockchain: Option<Arc<RwLock<Blockchain>>>,
}

impl Node {
    pub fn new(config: NodeConfig) -> Self {
        Self {
            config,
            status: NodeStatus::Starting,
            start_time: std::time::Instant::now(),
            blockchain: None,
        }
    }

    /// Elapsed seconds since the node was created.
    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    pub fn get_status(&self) -> &NodeStatus {
        &self.status
    }

    /// Return a snapshot of current node information.
    pub fn get_info(&self) -> NodeInfo {
        NodeInfo {
            chain_id: self.config.chain_id,
            rpc_port: self.config.rpc_port,
            uptime_secs: self.uptime_secs(),
            status: self.status.to_string(),
            version: "0.1.0".to_string(),
            persist: self.config.persist,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> NodeConfig {
        NodeConfig::default()
    }

    #[test]
    fn test_node_config_defaults() {
        let cfg = NodeConfig::default();
        assert_eq!(cfg.rpc_port, 9944);
        assert_eq!(cfg.block_time_secs, 3);
        assert_eq!(cfg.data_dir, "./data");
        assert!(!cfg.persist);
        assert_eq!(cfg.max_txs_per_block, 100);
        assert!(!cfg.produce_empty_blocks);
        assert_eq!(cfg.chain_id, crate::types::constants::MAIN_CHAIN_ID);
    }

    #[test]
    fn test_node_config_from_env() {
        // Set env vars
        std::env::set_var("CHAIN_ID", "42");
        std::env::set_var("RPC_PORT", "8080");
        std::env::set_var("BLOCK_TIME_SECS", "6");
        std::env::set_var("DATA_DIR", "/tmp/test-data");
        std::env::set_var("PERSIST", "true");
        std::env::set_var("MAX_TXS_PER_BLOCK", "50");
        std::env::set_var("PRODUCE_EMPTY_BLOCKS", "1");

        let cfg = NodeConfig::from_env();

        // Clean up immediately so other tests aren't affected
        for key in &[
            "CHAIN_ID",
            "RPC_PORT",
            "BLOCK_TIME_SECS",
            "DATA_DIR",
            "PERSIST",
            "MAX_TXS_PER_BLOCK",
            "PRODUCE_EMPTY_BLOCKS",
        ] {
            std::env::remove_var(key);
        }

        assert_eq!(cfg.chain_id, 42);
        assert_eq!(cfg.rpc_port, 8080);
        assert_eq!(cfg.block_time_secs, 6);
        assert_eq!(cfg.data_dir, "/tmp/test-data");
        assert!(cfg.persist);
        assert_eq!(cfg.max_txs_per_block, 50);
        assert!(cfg.produce_empty_blocks);
    }

    #[test]
    fn test_node_new() {
        let cfg = default_config();
        let node = Node::new(cfg.clone());

        assert_eq!(*node.get_status(), NodeStatus::Starting);
        assert!(node.blockchain.is_none());
        assert_eq!(node.config.rpc_port, cfg.rpc_port);
    }

    #[test]
    fn test_node_info() {
        let cfg = default_config();
        let node = Node::new(cfg.clone());
        let info = node.get_info();

        assert_eq!(info.chain_id, cfg.chain_id);
        assert_eq!(info.rpc_port, cfg.rpc_port);
        assert_eq!(info.version, "0.1.0");
        assert_eq!(info.status, "starting");
        assert_eq!(info.persist, cfg.persist);
        // uptime should be very small but non-negative
        assert!(info.uptime_secs < 5);
    }

    #[test]
    fn test_node_status() {
        let cfg = default_config();
        let mut node = Node::new(cfg);

        assert_eq!(*node.get_status(), NodeStatus::Starting);

        node.status = NodeStatus::Running;
        assert_eq!(*node.get_status(), NodeStatus::Running);

        node.status = NodeStatus::ShuttingDown;
        assert_eq!(*node.get_status(), NodeStatus::ShuttingDown);

        node.status = NodeStatus::Stopped;
        assert_eq!(*node.get_status(), NodeStatus::Stopped);

        // Display formatting
        assert_eq!(NodeStatus::Running.to_string(), "running");
        assert_eq!(NodeStatus::ShuttingDown.to_string(), "shutting_down");
        assert_eq!(NodeStatus::Stopped.to_string(), "stopped");
        assert_eq!(NodeStatus::Starting.to_string(), "starting");
    }
}
