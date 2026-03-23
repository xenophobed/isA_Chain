use crate::keystore::{Keystore, KeystoreType};
use crate::account::{WalletAccount, AccountType};
use crate::mnemonic::Mnemonic;
use crate::storage::{WalletStorage, Storage};
use crate::error::*;
use crate::crypto::EncryptionKey;

use isa_chain_core::types::*;
use isa_chain_core::transaction::{Transaction, TransactionData};
use isa_chain_core::crypto::KeyPair;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Main wallet structure
pub struct Wallet {
    /// Wallet ID
    pub id: String,

    /// Wallet name
    pub name: String,

    /// Wallet type
    pub wallet_type: WalletType,

    /// Encrypted keystore
    keystore: Keystore,

    /// Accounts derived from keystore
    accounts: HashMap<Address, WalletAccount>,

    /// Storage backend
    storage: Box<dyn Storage>,

    /// Current account index for derivation
    account_index: u32,

    /// Network configuration
    network: NetworkConfig,
}

impl std::fmt::Debug for Wallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wallet")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("wallet_type", &self.wallet_type)
            .field("account_index", &self.account_index)
            .finish_non_exhaustive()
    }
}

/// Wallet types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WalletType {
    /// HD wallet derived from mnemonic
    Hierarchical,
    /// Single key wallet
    SingleKey,
    /// Multi-signature wallet
    MultiSig {
        threshold: u32,
        total_signers: u32,
    },
    /// Hardware wallet
    Hardware {
        device_type: HardwareType,
        device_id: String,
    },
    /// Watch-only wallet
    WatchOnly,
}

/// Hardware wallet types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HardwareType {
    Ledger,
    Trezor,
    KeepKey,
    ColdCard,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub chain_id: ChainId,
    pub network_name: String,
    pub rpc_url: String,
    pub explorer_url: String,
    pub currency_symbol: String,
    pub currency_decimals: u8,
}

/// Wallet creation parameters
#[derive(Debug, Clone)]
pub struct WalletParams {
    pub name: String,
    pub password: String,
    pub wallet_type: WalletType,
    pub network: NetworkConfig,
    pub mnemonic: Option<String>,
    pub entropy_length: Option<usize>,
}

/// Transaction signing parameters
#[derive(Debug, Clone)]
pub struct SigningParams {
    pub from: Address,
    pub password: String,
    pub transaction: Transaction,
}

impl Wallet {
    /// Create a new wallet
    pub async fn create(params: WalletParams) -> Result<Self, WalletError> {
        let wallet_id = uuid::Uuid::new_v4().to_string();
        
        // Generate or import mnemonic
        let mnemonic = match params.mnemonic {
            Some(words) => Mnemonic::from_phrase(&words)?,
            None => Mnemonic::generate(params.entropy_length.unwrap_or(128))?,
        };
        
        // Create keystore
        let keystore = match params.wallet_type {
            WalletType::Hierarchical => {
                Keystore::from_mnemonic(
                    mnemonic,
                    &params.password,
                    KeystoreType::Encrypted,
                )?
            }
            WalletType::SingleKey => {
                // Generate single key
                let keypair = KeyPair::generate()
                    .map_err(|e| WalletError::CryptoError(e.to_string()))?;
                Keystore::from_private_key(
                    keypair.private_key,
                    &params.password,
                    KeystoreType::Encrypted,
                )?
            }
            WalletType::MultiSig { .. } => {
                return Err(WalletError::UnsupportedWalletType("MultiSig not yet implemented".to_string()));
            }
            WalletType::Hardware { .. } => {
                return Err(WalletError::UnsupportedWalletType("Hardware wallets not yet implemented".to_string()));
            }
            WalletType::WatchOnly => {
                return Err(WalletError::UnsupportedWalletType("Watch-only wallets not yet implemented".to_string()));
            }
        };
        
        // Create storage
        let storage_path = format!("wallets/{}", wallet_id);
        let storage: Box<dyn Storage> = Box::new(WalletStorage::new(&storage_path)?);
        
        let mut wallet = Wallet {
            id: wallet_id,
            name: params.name,
            wallet_type: params.wallet_type,
            keystore,
            accounts: HashMap::new(),
            storage,
            account_index: 0,
            network: params.network,
        };
        
        // Generate first account
        wallet.derive_next_account().await?;
        
        // Save wallet to storage
        wallet.save().await?;
        
        Ok(wallet)
    }
    
    /// Load wallet from storage
    pub async fn load(wallet_id: &str, password: &str) -> Result<Self, WalletError> {
        let storage_path = format!("wallets/{}", wallet_id);
        let storage: Box<dyn Storage> = Box::new(WalletStorage::new(&storage_path)?);
        
        // Load wallet metadata
        let wallet_data = storage.load_wallet_data().await?;
        
        // Decrypt keystore
        let keystore = Keystore::load(&wallet_data.keystore_data, password)?;
        
        // Load accounts
        let accounts = storage.load_accounts().await?;
        
        Ok(Wallet {
            id: wallet_id.to_string(),
            name: wallet_data.name,
            wallet_type: wallet_data.wallet_type,
            keystore,
            accounts,
            storage,
            account_index: wallet_data.account_index,
            network: wallet_data.network,
        })
    }
    
    /// Derive next account
    pub async fn derive_next_account(&mut self) -> Result<WalletAccount, WalletError> {
        let derivation_path = format!("m/44'/60'/0'/0/{}", self.account_index);
        
        let keypair = self.keystore.derive_key(&derivation_path)?;
        
        let account = WalletAccount::new(
            keypair.address,
            AccountType::External,
            derivation_path,
            format!("Account {}", self.account_index + 1),
            0, // Initial balance
        );
        
        self.accounts.insert(account.address, account.clone());
        self.account_index += 1;
        
        // Save updated wallet
        self.save().await?;
        
        Ok(account)
    }
    
    /// Get account by address
    pub fn get_account(&self, address: &Address) -> Option<&WalletAccount> {
        self.accounts.get(address)
    }
    
    /// Get all accounts
    pub fn get_accounts(&self) -> Vec<&WalletAccount> {
        self.accounts.values().collect()
    }
    
    /// Get primary account (first account)
    pub fn get_primary_account(&self) -> Option<&WalletAccount> {
        self.accounts.values().next()
    }
    
    /// Sign transaction
    pub async fn sign_transaction(&self, params: SigningParams) -> Result<Transaction, WalletError> {
        // Verify account exists
        let account = self.get_account(&params.from)
            .ok_or(WalletError::AccountNotFound(params.from))?;
        
        // Get private key for account
        let private_key = self.keystore.get_private_key(&account.derivation_path, &params.password)?;
        
        // Sign transaction
        let mut transaction = params.transaction;
        transaction.sign(&private_key)
            .map_err(|e| WalletError::SigningError(e.to_string()))?;
        
        Ok(transaction)
    }
    
    /// Create and sign a transfer transaction
    pub async fn create_transfer(
        &self,
        from: Address,
        to: Address,
        amount: Amount,
        gas_limit: Gas,
        gas_price: GasPrice,
        password: &str,
    ) -> Result<Transaction, WalletError> {
        // Get account nonce
        let account = self.get_account(&from)
            .ok_or(WalletError::AccountNotFound(from))?;
        
        // Create transaction
        let mut transaction = Transaction::new(
            from,
            account.nonce,
            TransactionData::Transfer {
                to,
                amount,
                data: vec![],
            },
            gas_limit,
            gas_price,
            self.network.chain_id,
        );
        
        // Sign transaction
        let private_key = self.keystore.get_private_key(&account.derivation_path, password)?;
        transaction.sign(&private_key)
            .map_err(|e| WalletError::SigningError(e.to_string()))?;
        
        Ok(transaction)
    }
    
    /// Update account balance
    pub async fn update_account_balance(&mut self, address: Address, balance: Amount) -> Result<(), WalletError> {
        if let Some(account) = self.accounts.get_mut(&address) {
            account.balance = balance;
            self.save().await?;
        }
        Ok(())
    }
    
    /// Update account nonce
    pub async fn update_account_nonce(&mut self, address: Address, nonce: u64) -> Result<(), WalletError> {
        if let Some(account) = self.accounts.get_mut(&address) {
            account.nonce = nonce;
            self.save().await?;
        }
        Ok(())
    }
    
    /// Export wallet (mnemonic or private key)
    pub fn export(&self, password: &str) -> Result<WalletExport, WalletError> {
        match self.wallet_type {
            WalletType::Hierarchical => {
                let mnemonic = self.keystore.export_mnemonic(password)?;
                Ok(WalletExport::Mnemonic(mnemonic.to_string()))
            }
            WalletType::SingleKey => {
                let private_key = self.keystore.get_master_private_key(password)?;
                Ok(WalletExport::PrivateKey(hex::encode(private_key)))
            }
            _ => Err(WalletError::UnsupportedOperation("Export not supported for this wallet type".to_string())),
        }
    }
    
    /// Change wallet password
    pub async fn change_password(&mut self, old_password: &str, new_password: &str) -> Result<(), WalletError> {
        // Verify old password by attempting to decrypt
        self.keystore.verify_password(old_password)?;
        
        // Re-encrypt with new password
        self.keystore.change_password(old_password, new_password)?;
        
        // Save updated keystore
        self.save().await?;
        
        Ok(())
    }
    
    /// Delete wallet
    pub async fn delete(self) -> Result<(), WalletError> {
        self.storage.delete_all().await?;
        Ok(())
    }
    
    /// Save wallet to storage
    async fn save(&self) -> Result<(), WalletError> {
        let wallet_data = WalletData {
            name: self.name.clone(),
            wallet_type: self.wallet_type.clone(),
            keystore_data: self.keystore.export()?,
            account_index: self.account_index,
            network: self.network.clone(),
        };
        
        self.storage.save_wallet_data(&wallet_data).await?;
        self.storage.save_accounts(&self.accounts).await?;
        
        Ok(())
    }
    
    /// Get wallet info
    pub fn get_info(&self) -> WalletInfo {
        WalletInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            wallet_type: self.wallet_type.clone(),
            account_count: self.accounts.len(),
            network: self.network.clone(),
            total_balance: self.accounts.values().map(|a| a.balance).sum(),
        }
    }
}

/// Wallet export formats
#[derive(Debug, Clone)]
pub enum WalletExport {
    Mnemonic(String),
    PrivateKey(String),
    Keystore(String),
}

/// Wallet storage data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletData {
    pub name: String,
    pub wallet_type: WalletType,
    pub keystore_data: Vec<u8>,
    pub account_index: u32,
    pub network: NetworkConfig,
}

/// Wallet information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    pub id: String,
    pub name: String,
    pub wallet_type: WalletType,
    pub account_count: usize,
    pub network: NetworkConfig,
    pub total_balance: Amount,
}

/// Secure memory management for sensitive data
#[derive(ZeroizeOnDrop)]
pub struct SecureString(String);

impl SecureString {
    pub fn new(s: String) -> Self {
        SecureString(s)
    }
    
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SecureString {
    fn from(s: String) -> Self {
        SecureString(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_wallet_creation() {
        let params = WalletParams {
            name: "Test Wallet".to_string(),
            password: "test123".to_string(),
            wallet_type: WalletType::Hierarchical,
            network: NetworkConfig {
                chain_id: 1,
                network_name: "Test Network".to_string(),
                rpc_url: "http://localhost:8545".to_string(),
                explorer_url: "http://localhost:3000".to_string(),
                currency_symbol: "ISA".to_string(),
                currency_decimals: 18,
            },
            mnemonic: None,
            entropy_length: Some(128),
        };
        
        let wallet = Wallet::create(params).await.unwrap();
        
        assert_eq!(wallet.name, "Test Wallet");
        assert_eq!(wallet.wallet_type, WalletType::Hierarchical);
        assert!(!wallet.accounts.is_empty());
    }
    
    #[tokio::test]
    async fn test_account_derivation() {
        let params = WalletParams {
            name: "Test Wallet".to_string(),
            password: "test123".to_string(),
            wallet_type: WalletType::Hierarchical,
            network: NetworkConfig {
                chain_id: 1,
                network_name: "Test Network".to_string(),
                rpc_url: "http://localhost:8545".to_string(),
                explorer_url: "http://localhost:3000".to_string(),
                currency_symbol: "ISA".to_string(),
                currency_decimals: 18,
            },
            mnemonic: None,
            entropy_length: Some(128),
        };
        
        let mut wallet = Wallet::create(params).await.unwrap();
        let initial_count = wallet.accounts.len();
        
        let new_account = wallet.derive_next_account().await.unwrap();
        assert_eq!(wallet.accounts.len(), initial_count + 1);
        assert!(wallet.accounts.contains_key(&new_account.address));
    }
}