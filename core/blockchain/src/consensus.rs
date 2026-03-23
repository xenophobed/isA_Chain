use crate::types::*;
use crate::error::*;
use crate::blockchain::Blockchain;
use crate::pos::PoSConsensus;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};
use tracing::{info, error, warn};

/// Consensus mechanism implementation backed by PoS.
pub struct ConsensusEngine {
    /// Current consensus round
    pub round: u32,

    /// Current block height being proposed
    pub height: BlockHeight,

    /// Proof-of-Stake engine (validator set + proposer selection)
    pub pos: PoSConsensus,
}

impl ConsensusEngine {
    /// Create a new ConsensusEngine with a fresh PoS engine.
    ///
    /// Defaults: epoch_length = 100, min_validators = 1 (single-node mode).
    pub fn new() -> Self {
        ConsensusEngine {
            round: 0,
            height: 0,
            pos: PoSConsensus::new(100, 1),
        }
    }

    /// Register a validator with `stake` locked at `height`.
    ///
    /// Delegates directly to [`PoSConsensus::register_validator`].
    pub fn register_validator(
        &mut self,
        address: Address,
        stake: Amount,
        height: BlockHeight,
    ) -> Result<(), ConsensusError> {
        self.pos
            .register_validator(address, stake, height)
            .map_err(|_e| ConsensusError::InvalidValidatorSet)
    }

    /// Return the proposer for `height`.
    ///
    /// When no validators are registered (single-node / genesis mode) this
    /// returns `Address::ZERO` so the block producer can proceed without a
    /// full validator set.
    pub fn get_proposer(&mut self, height: BlockHeight) -> Result<Address, ConsensusError> {
        if self.pos.validators.is_empty() {
            // Single-node mode: no validators registered yet.
            return Ok(Address::ZERO);
        }
        self.pos
            .select_proposer(height)
            .map_err(|_e| ConsensusError::InvalidValidatorSet)
    }
}

impl Default for ConsensusEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// PoS-aware block producer.
pub struct BlockProducer {
    blockchain: Arc<RwLock<Blockchain>>,
    consensus: Arc<RwLock<ConsensusEngine>>,
    block_time: Duration,
    max_txs_per_block: usize,
    /// When `true`, produce blocks even when the mempool is empty (PoS
    /// liveness requirement).  Defaults to `false`.
    pub produce_empty_blocks: bool,
}

impl BlockProducer {
    pub fn new(
        blockchain: Arc<RwLock<Blockchain>>,
        consensus: Arc<RwLock<ConsensusEngine>>,
        block_time_secs: u64,
        max_txs_per_block: usize,
    ) -> Self {
        BlockProducer {
            blockchain,
            consensus,
            block_time: Duration::from_secs(block_time_secs),
            max_txs_per_block,
            produce_empty_blocks: false,
        }
    }

    /// Start producing blocks at regular intervals.
    pub async fn start(self) {
        info!("🏗️  Block producer started (block time: {:?})", self.block_time);

        let mut tick = interval(self.block_time);

        loop {
            tick.tick().await;

            match self.produce_block().await {
                Ok(block_hash) => {
                    let blockchain = self.blockchain.read().await;
                    info!(
                        "✅ Block produced: height={}, hash={}, txs={}",
                        blockchain.get_height(),
                        hex::encode(block_hash.as_bytes()),
                        blockchain.get_block(&block_hash)
                            .map(|b| b.transactions.len())
                            .unwrap_or(0)
                    );
                }
                Err(e) => {
                    // Only log error if there are pending transactions
                    let blockchain = self.blockchain.read().await;
                    let pending_count = blockchain.get_pending_transactions(1).len();
                    if pending_count > 0 {
                        error!("❌ Failed to produce block: {}", e);
                    }
                }
            }
        }
    }

    async fn produce_block(&self) -> Result<Hash, BlockchainError> {
        // ----------------------------------------------------------------
        // 1. Determine current height and check pending txs
        // ----------------------------------------------------------------
        let (pending_count, current_height) = {
            let blockchain = self.blockchain.read().await;
            let pending = blockchain.get_pending_transactions(1).len();
            let h = blockchain.get_height();
            (pending, h)
        };

        if pending_count == 0 && !self.produce_empty_blocks {
            // Nothing to do — skip empty block
            return Err(BlockchainError::Validation(
                crate::error::ValidationError::InvalidTransactionOrder,
            ));
        }

        // ----------------------------------------------------------------
        // 2. Ask the consensus engine for the proposer at next height
        // ----------------------------------------------------------------
        let next_height = current_height + 1;
        let proposer = {
            let mut consensus = self.consensus.write().await;
            consensus.get_proposer(next_height)?
        };

        // ----------------------------------------------------------------
        // 3. Produce the block
        // ----------------------------------------------------------------
        let block_hash = {
            let mut blockchain = self.blockchain.write().await;
            blockchain.produce_block(self.max_txs_per_block)?
        };

        // ----------------------------------------------------------------
        // 4. Record the block in PoS stats (best-effort; skip on error)
        // ----------------------------------------------------------------
        if proposer != Address::ZERO {
            let mut consensus = self.consensus.write().await;
            if let Err(e) = consensus.pos.record_block_proposed(&proposer) {
                warn!("Could not record block_proposed for {:?}: {}", proposer, e);
            }
        }

        Ok(block_hash)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::constants::VALIDATOR_MIN_STAKE;

    fn dummy_address(byte: u8) -> Address {
        let mut bytes = [0u8; 20];
        bytes[0] = byte;
        Address::from(bytes)
    }

    #[test]
    fn test_consensus_engine_new() {
        let engine = ConsensusEngine::new();
        assert_eq!(engine.round, 0);
        assert_eq!(engine.height, 0);
        assert_eq!(engine.pos.validators.len(), 0);
        assert_eq!(engine.pos.epoch_length, 100);
        assert_eq!(engine.pos.min_validators, 1);
    }

    #[test]
    fn test_register_validator() {
        let mut engine = ConsensusEngine::new();
        let addr = dummy_address(1);
        let stake = VALIDATOR_MIN_STAKE;

        assert!(engine.register_validator(addr, stake, 0).is_ok());
        assert_eq!(engine.pos.validators.len(), 1);
        assert_eq!(engine.pos.validators[0].address, addr);
    }

    #[test]
    fn test_get_proposer_no_validators_returns_zero() {
        let mut engine = ConsensusEngine::new();
        // Single-node mode: no validators → should not error, returns ZERO
        let result = engine.get_proposer(1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Address::ZERO);
    }

    #[test]
    fn test_get_proposer_with_validator() {
        let mut engine = ConsensusEngine::new();
        let addr = dummy_address(2);
        engine.register_validator(addr, VALIDATOR_MIN_STAKE, 0).unwrap();

        let result = engine.get_proposer(1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), addr);
    }

    #[test]
    fn test_block_producer_with_consensus() {
        // Verify the struct compiles and fields are accessible.
        let blockchain = Arc::new(RwLock::new(Blockchain::new(1)));
        let consensus = Arc::new(RwLock::new(ConsensusEngine::new()));
        let producer = BlockProducer::new(
            blockchain,
            consensus,
            5,
            100,
        );
        assert!(!producer.produce_empty_blocks);
        assert_eq!(producer.block_time, Duration::from_secs(5));
        assert_eq!(producer.max_txs_per_block, 100);
    }
}
