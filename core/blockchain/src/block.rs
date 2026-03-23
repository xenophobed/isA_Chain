use crate::types::*;
use crate::transaction::Transaction;
use serde::{Deserialize, Serialize};

/// Block header containing metadata
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BlockHeader {
    /// Block height/number
    pub height: BlockHeight,
    
    /// Hash of the previous block
    pub parent_hash: Hash,
    
    /// Merkle root of all transactions in this block
    pub transactions_root: Hash,
    
    /// State root after applying all transactions
    pub state_root: Hash,
    
    /// Receipts root (for transaction receipts)
    pub receipts_root: Hash,
    
    /// Block timestamp (Unix timestamp in milliseconds)
    pub timestamp: Timestamp,
    
    /// Gas limit for this block
    pub gas_limit: Gas,
    
    /// Total gas used by all transactions
    pub gas_used: Gas,
    
    /// Address of the block producer/validator
    pub proposer: Address,
    
    /// Extra data (up to 32 bytes)
    pub extra_data: Vec<u8>,
    
    /// Consensus-related fields
    pub consensus_data: ConsensusData,
}

/// Consensus-specific data in block header
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConsensusData {
    /// Validator signatures for this block
    pub validator_signatures: Vec<ValidatorSignature>,
    
    /// Proof-of-stake specific data
    pub stake_data: StakeData,
    
    /// Random beacon value
    pub randomness: Hash,
}

/// Validator signature for consensus
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ValidatorSignature {
    pub validator: Address,
    pub signature: Signature,
    pub voting_power: Amount,
}

/// Proof-of-stake related data
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StakeData {
    /// Total stake of all validators
    pub total_stake: Amount,
    
    /// Minimum stake required for validation
    pub min_stake: Amount,
    
    /// Slash penalties applied in this block
    pub slash_penalties: Vec<SlashPenalty>,
}

/// Slash penalty for misbehaving validator
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SlashPenalty {
    pub validator: Address,
    pub penalty_type: SlashType,
    pub amount: Amount,
    pub evidence_hash: Hash,
}

/// Types of slashing penalties
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SlashType {
    DoubleSign,
    Downtime,
    InvalidProposal,
    EquivocatingVote,
}

/// Complete block with header and transactions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    /// Block header
    pub header: BlockHeader,
    
    /// List of transactions in this block
    pub transactions: Vec<Transaction>,
    
    /// Block hash (computed from header)
    pub hash: Hash,
}

impl Block {
    /// Create a new block
    pub fn new(
        height: BlockHeight,
        parent_hash: Hash,
        transactions: Vec<Transaction>,
        state_root: Hash,
        receipts_root: Hash,
        proposer: Address,
        gas_limit: Gas,
        consensus_data: ConsensusData,
    ) -> Self {
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;
        
        // Calculate gas used
        let gas_used = transactions.iter()
            .map(|tx| tx.gas_limit)
            .sum();
        
        // Calculate transactions root (Merkle root of transaction hashes)
        let tx_hashes: Vec<Hash> = transactions.iter()
            .map(|tx| tx.hash())
            .collect();
        let transactions_root = Hash::merkle_root(&tx_hashes);
        
        let header = BlockHeader {
            height,
            parent_hash,
            transactions_root,
            state_root,
            receipts_root,
            timestamp,
            gas_limit,
            gas_used,
            proposer,
            extra_data: Vec::new(),
            consensus_data,
        };
        
        let hash = Self::compute_hash(&header);
        
        Block {
            header,
            transactions,
            hash,
        }
    }
    
    /// Compute block hash from header
    pub fn compute_hash(header: &BlockHeader) -> Hash {
        let serialized = bincode::serialize(header)
            .expect("Block header serialization should never fail");
        Hash::hash_data(&serialized)
    }
    
    /// Get block hash
    pub fn hash(&self) -> Hash {
        self.hash
    }
    
    /// Verify block integrity
    pub fn verify(&self) -> Result<(), BlockError> {
        // Verify hash matches header
        let computed_hash = Self::compute_hash(&self.header);
        if computed_hash != self.hash {
            return Err(BlockError::InvalidHash {
                expected: computed_hash,
                actual: self.hash,
            });
        }
        
        // Verify transactions root
        let tx_hashes: Vec<Hash> = self.transactions.iter()
            .map(|tx| tx.hash())
            .collect();
        let transactions_root = Hash::merkle_root(&tx_hashes);
        if transactions_root != self.header.transactions_root {
            return Err(BlockError::InvalidTransactionsRoot {
                expected: transactions_root,
                actual: self.header.transactions_root,
            });
        }
        
        // Verify gas usage
        let total_gas_used: Gas = self.transactions.iter()
            .map(|tx| tx.gas_limit)
            .sum();
        if total_gas_used != self.header.gas_used {
            return Err(BlockError::InvalidGasUsed {
                expected: total_gas_used,
                actual: self.header.gas_used,
            });
        }
        
        // Verify gas limit
        if self.header.gas_used > self.header.gas_limit {
            return Err(BlockError::GasLimitExceeded {
                limit: self.header.gas_limit,
                used: self.header.gas_used,
            });
        }
        
        // Verify timestamp is reasonable (not too far in future)
        let now = chrono::Utc::now().timestamp_millis() as u64;
        if self.header.timestamp > now + 60_000 { // Allow 1 minute drift
            return Err(BlockError::InvalidTimestamp {
                timestamp: self.header.timestamp,
                now,
            });
        }
        
        // Verify individual transactions
        for tx in &self.transactions {
            tx.verify().map_err(BlockError::InvalidTransaction)?;
        }
        
        Ok(())
    }
    
    /// Check if this block is valid as the next block after parent
    pub fn is_valid_successor(&self, parent: &Block) -> Result<(), BlockError> {
        // Height should be parent + 1
        if self.header.height != parent.header.height + 1 {
            return Err(BlockError::InvalidHeight {
                expected: parent.header.height + 1,
                actual: self.header.height,
            });
        }
        
        // Parent hash should match
        if self.header.parent_hash != parent.hash {
            return Err(BlockError::InvalidParentHash {
                expected: parent.hash,
                actual: self.header.parent_hash,
            });
        }
        
        // Timestamp should be after parent
        if self.header.timestamp <= parent.header.timestamp {
            return Err(BlockError::InvalidTimestamp {
                timestamp: self.header.timestamp,
                now: parent.header.timestamp,
            });
        }
        
        Ok(())
    }
    
    /// Get block size in bytes
    pub fn size(&self) -> usize {
        bincode::serialize(self)
            .map(|data| data.len())
            .unwrap_or(0)
    }
    
    /// Check if block is full (reached size or gas limit)
    pub fn is_full(&self) -> bool {
        self.size() >= constants::MAX_BLOCK_SIZE || 
        self.header.gas_used >= self.header.gas_limit
    }
    
    /// Get transaction by hash
    pub fn get_transaction(&self, tx_hash: &Hash) -> Option<&Transaction> {
        self.transactions.iter()
            .find(|tx| tx.hash() == *tx_hash)
    }
}

/// Genesis block creation
impl Block {
    /// Create the genesis block
    pub fn genesis(chain_id: ChainId) -> Self {
        let genesis_address = Address::ZERO;
        let genesis_transactions = vec![]; // Genesis block has no transactions
        
        let consensus_data = ConsensusData {
            validator_signatures: vec![],
            stake_data: StakeData {
                total_stake: 0,
                min_stake: constants::VALIDATOR_MIN_STAKE,
                slash_penalties: vec![],
            },
            randomness: Hash::hash_data(b"genesis_randomness"),
        };
        
        let mut genesis = Block::new(
            0, // Genesis height is 0
            Hash::ZERO, // No parent
            genesis_transactions,
            Hash::hash_data(b"genesis_state"), // Initial state root
            Hash::ZERO, // No receipts
            genesis_address,
            constants::MAX_GAS_PER_BLOCK,
            consensus_data,
        );
        
        // Set genesis timestamp
        genesis.header.timestamp = constants::GENESIS_TIMESTAMP;
        
        // Recompute hash with correct timestamp
        genesis.hash = Self::compute_hash(&genesis.header);
        
        genesis
    }
}

/// Block validation errors
#[derive(Debug, thiserror::Error)]
pub enum BlockError {
    #[error("Invalid block hash: expected {expected}, got {actual}")]
    InvalidHash { expected: Hash, actual: Hash },
    
    #[error("Invalid transactions root: expected {expected}, got {actual}")]
    InvalidTransactionsRoot { expected: Hash, actual: Hash },
    
    #[error("Invalid gas used: expected {expected}, got {actual}")]
    InvalidGasUsed { expected: Gas, actual: Gas },
    
    #[error("Gas limit exceeded: limit {limit}, used {used}")]
    GasLimitExceeded { limit: Gas, used: Gas },
    
    #[error("Invalid timestamp: {timestamp}, now: {now}")]
    InvalidTimestamp { timestamp: Timestamp, now: Timestamp },
    
    #[error("Invalid block height: expected {expected}, got {actual}")]
    InvalidHeight { expected: BlockHeight, actual: BlockHeight },
    
    #[error("Invalid parent hash: expected {expected}, got {actual}")]
    InvalidParentHash { expected: Hash, actual: Hash },
    
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(crate::transaction::TransactionError),
    
    #[error("Block too large: {size} bytes")]
    BlockTooLarge { size: usize },
    
    #[error("Insufficient validator signatures")]
    InsufficientSignatures,
    
    #[error("Invalid consensus data: {reason}")]
    InvalidConsensus { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::{TransactionData, TransactionType};
    
    #[test]
    fn test_genesis_block_creation() {
        let genesis = Block::genesis(constants::MAIN_CHAIN_ID);
        
        assert_eq!(genesis.header.height, 0);
        assert_eq!(genesis.header.parent_hash, Hash::ZERO);
        assert_eq!(genesis.transactions.len(), 0);
        assert_eq!(genesis.header.timestamp, constants::GENESIS_TIMESTAMP);
        
        // Genesis block should be valid
        assert!(genesis.verify().is_ok());
    }
    
    #[test]
    fn test_block_creation_and_verification() {
        // Create a properly signed transaction for testing
        let private_key = [1u8; 32];

        // Derive the correct address from the private key
        let secp = secp256k1::Secp256k1::new();
        let secret_key = secp256k1::SecretKey::from_slice(&private_key).unwrap();
        let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
        let from = Address::from_public_key(&public_key.serialize());

        let mut tx = Transaction::new(
            from,
            0, // nonce
            TransactionData::Transfer {
                to: Address::from([2u8; 20]),
                amount: 1000,
                data: vec![],
            },
            21000, // gas_limit
            constants::BASE_GAS_PRICE,
            constants::MAIN_CHAIN_ID,
        );

        // Sign the transaction
        tx.sign(&private_key).expect("Signing should succeed");

        let consensus_data = ConsensusData {
            validator_signatures: vec![],
            stake_data: StakeData {
                total_stake: 1000000,
                min_stake: constants::VALIDATOR_MIN_STAKE,
                slash_penalties: vec![],
            },
            randomness: Hash::hash_data(b"test_randomness"),
        };

        let block = Block::new(
            1,
            Hash::hash_data(b"parent_hash"),
            vec![tx],
            Hash::hash_data(b"state_root"),
            Hash::hash_data(b"receipts_root"),
            Address::from([3u8; 20]),
            constants::MAX_GAS_PER_BLOCK,
            consensus_data,
        );

        assert!(block.verify().is_ok());
    }
    
    #[test]
    fn test_block_succession() {
        let parent = Block::genesis(constants::MAIN_CHAIN_ID);
        
        let child = Block::new(
            1,
            parent.hash(),
            vec![],
            Hash::hash_data(b"new_state_root"),
            Hash::ZERO,
            Address::from([1u8; 20]),
            constants::MAX_GAS_PER_BLOCK,
            ConsensusData {
                validator_signatures: vec![],
                stake_data: StakeData {
                    total_stake: 0,
                    min_stake: constants::VALIDATOR_MIN_STAKE,
                    slash_penalties: vec![],
                },
                randomness: Hash::hash_data(b"child_randomness"),
            },
        );
        
        assert!(child.is_valid_successor(&parent).is_ok());
    }
}