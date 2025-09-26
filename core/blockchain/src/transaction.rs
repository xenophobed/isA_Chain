use crate::types::*;
use serde::{Deserialize, Serialize};

/// Transaction types supported by the blockchain
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TransactionType {
    /// Simple token transfer
    Transfer,
    /// Smart contract deployment
    ContractDeploy,
    /// Smart contract function call
    ContractCall,
    /// Validator staking
    Stake,
    /// Unstaking tokens
    Unstake,
    /// Delegate to validator
    Delegate,
    /// Undelegate from validator
    Undelegate,
    /// Governance proposal submission
    GovernanceProposal,
    /// Governance vote
    GovernanceVote,
    /// Cross-chain bridge transaction
    Bridge,
}

/// Transaction data based on transaction type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TransactionData {
    /// Transfer tokens to another address
    Transfer {
        to: Address,
        amount: Amount,
        data: Vec<u8>, // Optional data payload
    },
    
    /// Deploy a new smart contract
    ContractDeploy {
        bytecode: Vec<u8>,
        constructor_args: Vec<u8>,
        salt: Option<Hash>, // For deterministic deployment
    },
    
    /// Call a smart contract function
    ContractCall {
        contract: Address,
        function_selector: [u8; 4],
        args: Vec<u8>,
        value: Amount, // Value to send with call
    },
    
    /// Stake tokens to become a validator
    Stake {
        amount: Amount,
        validator_info: ValidatorInfo,
    },
    
    /// Unstake tokens and stop being a validator
    Unstake {
        amount: Amount,
        validator: Address,
    },
    
    /// Delegate tokens to a validator
    Delegate {
        validator: Address,
        amount: Amount,
    },
    
    /// Undelegate tokens from a validator
    Undelegate {
        validator: Address,
        amount: Amount,
    },
    
    /// Submit a governance proposal
    GovernanceProposal {
        title: String,
        description: String,
        proposal_type: ProposalType,
        execution_data: Vec<u8>,
    },
    
    /// Vote on a governance proposal
    GovernanceVote {
        proposal_id: Hash,
        vote: VoteType,
        voting_power: Amount,
    },
    
    /// Cross-chain bridge transaction
    Bridge {
        target_chain: ChainId,
        target_address: Vec<u8>,
        amount: Amount,
        bridge_data: Vec<u8>,
    },
}

/// Validator information for staking
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ValidatorInfo {
    pub public_key: Vec<u8>,
    pub commission_rate: u32, // Basis points (10000 = 100%)
    pub min_self_delegation: Amount,
    pub description: ValidatorDescription,
}

/// Validator description metadata
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ValidatorDescription {
    pub moniker: String,
    pub identity: String,
    pub website: String,
    pub security_contact: String,
    pub details: String,
}

/// Governance proposal types
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ProposalType {
    TextProposal,
    ParameterChange,
    SoftwareUpgrade,
    ValidatorSlash,
    Treasury,
}

/// Vote types for governance
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum VoteType {
    Yes,
    No,
    Abstain,
    NoWithVeto,
}

/// Complete transaction structure
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transaction {
    /// Transaction sender
    pub from: Address,
    
    /// Transaction nonce (prevents replay attacks)
    pub nonce: u64,
    
    /// Transaction data/payload
    pub data: TransactionData,
    
    /// Maximum gas to use for this transaction
    pub gas_limit: Gas,
    
    /// Gas price (amount to pay per gas unit)
    pub gas_price: GasPrice,
    
    /// Chain ID (prevents cross-chain replay attacks)
    pub chain_id: ChainId,
    
    /// Transaction signature
    pub signature: Option<Signature>,
    
    /// Cached transaction hash
    hash: Option<Hash>,
}

impl Transaction {
    /// Create a new unsigned transaction
    pub fn new(
        from: Address,
        nonce: u64,
        data: TransactionData,
        gas_limit: Gas,
        gas_price: GasPrice,
        chain_id: ChainId,
    ) -> Self {
        Transaction {
            from,
            nonce,
            data,
            gas_limit,
            gas_price,
            chain_id,
            signature: None,
            hash: None,
        }
    }
    
    /// Get transaction hash
    pub fn hash(&self) -> Hash {
        if let Some(hash) = self.hash {
            return hash;
        }
        
        // Create a version of the transaction without the hash for hashing
        let mut tx_for_hash = self.clone();
        tx_for_hash.hash = None;
        
        let serialized = bincode::serialize(&tx_for_hash)
            .expect("Transaction serialization should never fail");
        Hash::hash_data(&serialized)
    }
    
    /// Get transaction hash for signing (excludes signature)
    pub fn signing_hash(&self) -> Hash {
        let mut tx_for_signing = self.clone();
        tx_for_signing.signature = None;
        tx_for_signing.hash = None;
        
        let serialized = bincode::serialize(&tx_for_signing)
            .expect("Transaction serialization should never fail");
        Hash::hash_data(&serialized)
    }
    
    /// Sign the transaction with a private key
    pub fn sign(&mut self, private_key: &[u8]) -> Result<(), TransactionError> {
        if private_key.len() != 32 {
            return Err(TransactionError::InvalidPrivateKey);
        }
        
        let signing_hash = self.signing_hash();
        
        // Use secp256k1 for signing (Ethereum-compatible)
        let secp = secp256k1::Secp256k1::new();
        let secret_key = secp256k1::SecretKey::from_slice(private_key)
            .map_err(|_| TransactionError::InvalidPrivateKey)?;
        
        let message = secp256k1::Message::from_digest_slice(signing_hash.as_bytes())
            .map_err(|_| TransactionError::SigningFailed)?;
        
        let signature = secp.sign_ecdsa(&message, &secret_key);
        let signature_bytes = signature.serialize_compact();
        
        // Split signature into r, s components
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&signature_bytes[0..32]);
        s.copy_from_slice(&signature_bytes[32..64]);
        
        // For now, use a default recovery ID (we'll need to compute this properly later)
        let recovery_id = 0u8;
        self.signature = Some(Signature::new(r, s, recovery_id));
        
        // Invalidate cached hash
        self.hash = None;
        
        Ok(())
    }
    
    /// Verify transaction signature
    pub fn verify(&self) -> Result<(), TransactionError> {
        let signature = self.signature.as_ref()
            .ok_or(TransactionError::MissingSignature)?;
        
        let signing_hash = self.signing_hash();
        
        // Verify signature
        let secp = secp256k1::Secp256k1::new();
        let message = secp256k1::Message::from_digest_slice(signing_hash.as_bytes())
            .map_err(|_| TransactionError::InvalidSignature)?;
        
        // Reconstruct signature
        let mut signature_bytes = [0u8; 64];
        signature_bytes[0..32].copy_from_slice(&signature.r);
        signature_bytes[32..64].copy_from_slice(&signature.s);
        
        let recovery_id = secp256k1::ecdsa::RecoveryId::from_i32(signature.v as i32)
            .map_err(|_| TransactionError::InvalidSignature)?;
        
        let recoverable_sig = secp256k1::ecdsa::RecoverableSignature::from_compact(
            &signature_bytes,
            recovery_id,
        ).map_err(|_| TransactionError::InvalidSignature)?;
        
        // Recover public key
        let public_key = secp.recover_ecdsa(&message, &recoverable_sig)
            .map_err(|_| TransactionError::InvalidSignature)?;
        
        // Verify the public key matches the from address
        let recovered_address = Address::from_public_key(&public_key.serialize());
        if recovered_address != self.from {
            return Err(TransactionError::InvalidSender {
                expected: self.from,
                recovered: recovered_address,
            });
        }
        
        // Additional validation checks
        self.validate_data()?;
        
        Ok(())
    }
    
    /// Validate transaction data based on type
    fn validate_data(&self) -> Result<(), TransactionError> {
        match &self.data {
            TransactionData::Transfer { amount, .. } => {
                if *amount == 0 {
                    return Err(TransactionError::InvalidAmount);
                }
            }
            
            TransactionData::ContractDeploy { bytecode, .. } => {
                if bytecode.is_empty() {
                    return Err(TransactionError::EmptyBytecode);
                }
                if bytecode.len() > 1_000_000 { // 1MB limit
                    return Err(TransactionError::BytecodeTooLarge);
                }
            }
            
            TransactionData::Stake { amount, validator_info } => {
                if *amount < constants::VALIDATOR_MIN_STAKE {
                    return Err(TransactionError::InsufficientStakeAmount);
                }
                if validator_info.commission_rate > 10000 {
                    return Err(TransactionError::InvalidCommissionRate);
                }
            }
            
            TransactionData::Delegate { amount, .. } => {
                if *amount < constants::DELEGATION_MIN_AMOUNT {
                    return Err(TransactionError::InsufficientDelegationAmount);
                }
            }
            
            _ => {} // Other transaction types are valid by default
        }
        
        Ok(())
    }
    
    /// Get the transaction type
    pub fn tx_type(&self) -> TransactionType {
        match &self.data {
            TransactionData::Transfer { .. } => TransactionType::Transfer,
            TransactionData::ContractDeploy { .. } => TransactionType::ContractDeploy,
            TransactionData::ContractCall { .. } => TransactionType::ContractCall,
            TransactionData::Stake { .. } => TransactionType::Stake,
            TransactionData::Unstake { .. } => TransactionType::Unstake,
            TransactionData::Delegate { .. } => TransactionType::Delegate,
            TransactionData::Undelegate { .. } => TransactionType::Undelegate,
            TransactionData::GovernanceProposal { .. } => TransactionType::GovernanceProposal,
            TransactionData::GovernanceVote { .. } => TransactionType::GovernanceVote,
            TransactionData::Bridge { .. } => TransactionType::Bridge,
        }
    }
    
    /// Calculate transaction fee
    pub fn fee(&self) -> Amount {
        (self.gas_limit as Amount) * (self.gas_price as Amount)
    }
    
    /// Get transaction size in bytes
    pub fn size(&self) -> usize {
        bincode::serialize(self)
            .map(|data| data.len())
            .unwrap_or(0)
    }
    
    /// Check if transaction is signed
    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }
}

/// Transaction validation errors
#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("Invalid private key")]
    InvalidPrivateKey,
    
    #[error("Signing failed")]
    SigningFailed,
    
    #[error("Missing signature")]
    MissingSignature,
    
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("Invalid sender: expected {expected}, recovered {recovered}")]
    InvalidSender { expected: Address, recovered: Address },
    
    #[error("Invalid amount")]
    InvalidAmount,
    
    #[error("Empty bytecode")]
    EmptyBytecode,
    
    #[error("Bytecode too large")]
    BytecodeTooLarge,
    
    #[error("Insufficient stake amount")]
    InsufficientStakeAmount,
    
    #[error("Insufficient delegation amount")]
    InsufficientDelegationAmount,
    
    #[error("Invalid commission rate")]
    InvalidCommissionRate,
    
    #[error("Invalid nonce: expected {expected}, got {actual}")]
    InvalidNonce { expected: u64, actual: u64 },
    
    #[error("Gas limit too low")]
    GasLimitTooLow,
    
    #[error("Gas price too low")]
    GasPriceTooLow,
    
    #[error("Insufficient balance")]
    InsufficientBalance,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_transaction_creation_and_signing() {
        let private_key = [1u8; 32];
        let from = Address::from([1u8; 20]);
        
        let mut tx = Transaction::new(
            from,
            0,
            TransactionData::Transfer {
                to: Address::from([2u8; 20]),
                amount: 1000,
                data: vec![],
            },
            21000,
            constants::BASE_GAS_PRICE,
            constants::MAIN_CHAIN_ID,
        );
        
        // Sign transaction
        assert!(tx.sign(&private_key).is_ok());
        assert!(tx.is_signed());
        
        // Verify transaction
        assert!(tx.verify().is_ok());
    }
    
    #[test]
    fn test_transaction_hash_consistency() {
        let tx = Transaction::new(
            Address::from([1u8; 20]),
            0,
            TransactionData::Transfer {
                to: Address::from([2u8; 20]),
                amount: 1000,
                data: vec![],
            },
            21000,
            constants::BASE_GAS_PRICE,
            constants::MAIN_CHAIN_ID,
        );
        
        let hash1 = tx.hash();
        let hash2 = tx.hash();
        
        assert_eq!(hash1, hash2);
    }
    
    #[test]
    fn test_invalid_stake_amount() {
        let tx = Transaction::new(
            Address::from([1u8; 20]),
            0,
            TransactionData::Stake {
                amount: 1000, // Too low
                validator_info: ValidatorInfo {
                    public_key: vec![1u8; 33],
                    commission_rate: 1000,
                    min_self_delegation: 1000,
                    description: ValidatorDescription {
                        moniker: "test".to_string(),
                        identity: "".to_string(),
                        website: "".to_string(),
                        security_contact: "".to_string(),
                        details: "".to_string(),
                    },
                },
            },
            21000,
            constants::BASE_GAS_PRICE,
            constants::MAIN_CHAIN_ID,
        );
        
        assert!(tx.validate_data().is_err());
    }
}