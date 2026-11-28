use crate::keystore::{Keystore, KeystoreType};
use crate::account::{WalletAccount, AccountType};
use crate::mnemonic::Mnemonic;
use crate::storage::{WalletStorage, Storage};
use crate::error::*;

use isa_chain_core::types::*;
use isa_chain_core::transaction::{Transaction, TransactionData};
use isa_chain_core::crypto::KeyPair;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zeroize::{ZeroizeOnDrop, Zeroizing};

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
    storage: WalletStorage,

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

impl WalletType {
    fn account_path(&self, index: u32) -> Result<String, WalletError> {
        match self {
            Self::Hierarchical if index < (1 << 31) => Ok(format!("m/44'/60'/0'/0/{index}")),
            Self::SingleKey if index == 0 => Ok("m".into()),
            _ => Err(WalletError::UnsupportedOperation("Wallet cannot derive another account".into())),
        }
    }
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
#[derive(Clone)]
pub struct WalletParams {
    pub name: String,
    pub password: String,
    pub wallet_type: WalletType,
    pub network: NetworkConfig,
    pub mnemonic: Option<String>,
    pub entropy_length: Option<usize>,
}

/// Transaction signing parameters
#[derive(Clone)]
pub struct SigningParams {
    pub from: Address,
    pub password: String,
    pub transaction: Transaction,
}

impl std::fmt::Debug for WalletParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalletParams").field("wallet_type", &self.wallet_type).finish_non_exhaustive()
    }
}

impl std::fmt::Debug for SigningParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigningParams").field("from", &self.from).finish_non_exhaustive()
    }
}

impl Wallet {
    /// Create a new wallet
    pub async fn create(params: WalletParams) -> Result<Self, WalletError> {
        let wallet_id = uuid::Uuid::new_v4().to_string();
        
        // Generate or import mnemonic
        let mnemonic = match params.mnemonic {
            Some(words) => Mnemonic::from_phrase(&Zeroizing::new(words))?,
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
        let storage = WalletStorage::create_wallet(&wallet_id)?;
        
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
        
        Ok(wallet)
    }
    
    /// Load wallet from storage
    pub async fn load(wallet_id: &str, password: &str) -> Result<Self, WalletError> {
        let storage = WalletStorage::open_existing(&WalletStorage::wallet_path(wallet_id)?)?;
        
        // Load wallet metadata
        let wallet_data = storage.load_wallet_data().await?;
        
        // Decrypt keystore
        let keystore = Keystore::load(&wallet_data.keystore_data, password)?;
        
        // Load accounts
        let accounts = storage.load_accounts().await?;
        if accounts.is_empty() || accounts.len() != wallet_data.account_index as usize {
            return Err(WalletError::StorageError("Inconsistent wallet account count".into()));
        }
        for index in 0..wallet_data.account_index {
            let path = wallet_data.wallet_type.account_path(index)?;
            let address = keystore.derive_key(&path)?.address;
            if accounts.get(&address).map(|account| &account.derivation_path) != Some(&path) {
                return Err(WalletError::StorageError("Account does not match its recovery path".into()));
            }
        }
        
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
        let derivation_path = self.wallet_type.account_path(self.account_index)?;

        let keypair = self.keystore.derive_key(&derivation_path)?;
        
        let account = WalletAccount::new(
            keypair.address,
            AccountType::External,
            derivation_path,
            format!("Account {}", self.account_index + 1),
            0, // Initial balance
        );
        
        if self.accounts.contains_key(&account.address) {
            return Err(WalletError::StorageError("Derived account is already recorded".into()));
        }
        self.accounts.insert(account.address, account.clone());
        self.account_index += 1;
        if let Err(error) = self.save().await {
            self.accounts.remove(&account.address);
            self.account_index -= 1;
            return Err(error);
        }
        
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
        let primary = self.wallet_type.account_path(0).ok()?;
        self.accounts.values().find(|a| a.derivation_path == primary)
    }
    
    /// Sign transaction
    pub async fn sign_transaction(&self, params: SigningParams) -> Result<Transaction, WalletError> {
        // Verify account exists
        let account = self.get_account(&params.from)
            .ok_or(WalletError::AccountNotFound(params.from))?;
        
        if params.transaction.from != params.from {
            return Err(WalletError::SigningError("Sender does not match signing account".into()));
        }
        let private_key = Zeroizing::new(self.keystore.get_private_key(&account.derivation_path, &params.password)?);
        
        // Sign transaction
        let mut transaction = params.transaction;
        transaction.sign(&*private_key)
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
        let private_key = Zeroizing::new(self.keystore.get_private_key(&account.derivation_path, password)?);
        transaction.sign(&*private_key)
            .map_err(|e| WalletError::SigningError(e.to_string()))?;
        
        Ok(transaction)
    }
    
    /// Update account balance
    pub async fn update_account_balance(&mut self, address: Address, balance: Amount) -> Result<(), WalletError> {
        self.update_account(address, |account| account.balance = balance).await
    }

    /// Update account nonce
    pub async fn update_account_nonce(&mut self, address: Address, nonce: u64) -> Result<(), WalletError> {
        self.update_account(address, |account| account.nonce = nonce).await
    }

    async fn update_account(&mut self, address: Address, update: impl FnOnce(&mut WalletAccount)) -> Result<(), WalletError> {
        if let Some(account) = self.accounts.get_mut(&address) {
            let previous = account.clone();
            update(account);
            if let Err(error) = self.save().await {
                self.accounts.insert(address, previous);
                return Err(error);
            }
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
        let keystore = self.keystore.reencrypted(old_password, new_password)?;
        self.save_keystore(&keystore).await?;
        self.keystore = keystore;
        
        Ok(())
    }
    
    /// Delete wallet
    pub async fn delete(self) -> Result<(), WalletError> {
        self.storage.delete_all().await?;
        Ok(())
    }
    
    /// Save wallet to storage
    async fn save(&self) -> Result<(), WalletError> {
        self.save_keystore(&self.keystore).await
    }

    async fn save_keystore(&self, keystore: &Keystore) -> Result<(), WalletError> {
        let wallet_data = WalletData {
            name: self.name.clone(),
            wallet_type: self.wallet_type.clone(),
            keystore_data: keystore.export()?,
            account_index: self.account_index,
            network: self.network.clone(),
        };
        
        self.storage.save_snapshot(&wallet_data, &self.accounts).await
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
#[derive(Clone)]
pub enum WalletExport {
    Mnemonic(String),
    PrivateKey(String),
    Keystore(String),
}

impl std::fmt::Debug for WalletExport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalletExport").finish_non_exhaustive()
    }
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
        let created_path = WalletStorage::wallet_path(&wallet.id).unwrap();
        wallet.delete().await.unwrap();
        std::fs::remove_dir_all(created_path).unwrap();
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
        let created_path = WalletStorage::wallet_path(&wallet.id).unwrap();
        wallet.delete().await.unwrap();
        std::fs::remove_dir_all(created_path).unwrap();
    }
}