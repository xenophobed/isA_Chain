use crate::types::*;
use super::data::TransactionData;
use super::types::{Transaction, TransactionError, TransactionType};

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

        // Sign with recoverable signature to get the recovery ID
        let recoverable_sig = secp.sign_ecdsa_recoverable(&message, &secret_key);
        let (recovery_id, signature_bytes) = recoverable_sig.serialize_compact();

        // Split signature into r, s components
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&signature_bytes[0..32]);
        s.copy_from_slice(&signature_bytes[32..64]);

        self.signature = Some(Signature::new(r, s, recovery_id.to_i32() as u8));

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
            // Agent-native economy
            TransactionData::CreditPurchase { .. } => TransactionType::CreditPurchase,
            TransactionData::CreditSpend { .. } => TransactionType::CreditSpend,
            TransactionData::AgentWalletSpend { .. } => TransactionType::AgentWalletSpend,
            TransactionData::Settlement { .. } => TransactionType::Settlement,
            // Compute marketplace
            TransactionData::ComputeRegister { .. } => TransactionType::ComputeRegister,
            TransactionData::ComputeUpdateProvider { .. } => TransactionType::ComputeUpdateProvider,
            TransactionData::ComputeProviderExit { .. } => TransactionType::ComputeProviderExit,
            TransactionData::ComputeRequest { .. } => TransactionType::ComputeRequest,
            TransactionData::ComputeAccept { .. } => TransactionType::ComputeAccept,
            TransactionData::ComputeStart { .. } => TransactionType::ComputeStart,
            TransactionData::ComputeSettle { .. } => TransactionType::ComputeSettle,
            TransactionData::ComputeCancel { .. } => TransactionType::ComputeCancel,
            TransactionData::ComputeDispute { .. } => TransactionType::ComputeDispute,
            TransactionData::ComputeResolveDispute { .. } => TransactionType::ComputeResolveDispute,
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
