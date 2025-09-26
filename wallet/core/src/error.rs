use isa_chain_core::types::Address;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WalletError {
    #[error("Crypto error: {0}")]
    CryptoError(String),
    
    #[error("Invalid mnemonic: {0}")]
    InvalidMnemonic(String),
    
    #[error("Invalid entropy length: {0}")]
    InvalidEntropyLength(usize),
    
    #[error("Account not found: {0}")]
    AccountNotFound(Address),
    
    #[error("Signing error: {0}")]
    SigningError(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Unsupported wallet type: {0}")]
    UnsupportedWalletType(String),
    
    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),
    
    #[error("Invalid password")]
    InvalidPassword,
    
    #[error("Keystore error: {0}")]
    KeystoreError(String),
    
    #[error("Hardware wallet error: {0}")]
    HardwareError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
}