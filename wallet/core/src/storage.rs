use crate::account::WalletAccount;
use crate::error::WalletError;
use crate::wallet::WalletData;
use async_trait::async_trait;
use isa_chain_core::types::Address;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[async_trait]
pub trait Storage: Send + Sync + std::fmt::Debug {
    async fn save_wallet_data(&self, data: &WalletData) -> Result<(), WalletError>;
    async fn load_wallet_data(&self) -> Result<WalletData, WalletError>;
    async fn save_accounts(
        &self,
        accounts: &HashMap<Address, WalletAccount>,
    ) -> Result<(), WalletError>;
    async fn load_accounts(&self) -> Result<HashMap<Address, WalletAccount>, WalletError>;
    async fn delete_all(&self) -> Result<(), WalletError>;
}

const WALLET: &[u8] = b"wallet-v1";
const ACCOUNTS: &[u8] = b"accounts-v1";

#[derive(Debug)]
pub struct WalletStorage {
    db: sled::Db,
}

fn storage_error(error: impl std::fmt::Display) -> WalletError {
    WalletError::StorageError(error.to_string())
}

impl WalletStorage {
    pub(crate) fn wallet_path(id: &str) -> Result<PathBuf, WalletError> {
        let uuid = uuid::Uuid::parse_str(id).map_err(|_| storage_error("Invalid wallet ID"))?;
        let root = Path::new("wallets");
        match std::fs::symlink_metadata(root) {
            Ok(metadata) if !metadata.is_dir() => {
                return Err(storage_error("Invalid wallet directory"))
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(storage_error(error))
            }
            _ => {}
        }
        Ok(root.join(uuid.to_string()))
    }

    pub fn new(path: &str) -> Result<Self, WalletError> {
        Self::directory_builder()
            .recursive(true)
            .create(path)
            .map_err(storage_error)?;
        Self::open_db(Path::new(path))
    }

    fn directory_builder() -> std::fs::DirBuilder {
        #[allow(unused_mut)]
        let mut dirs = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            dirs.mode(0o700);
        }
        dirs
    }

    pub(crate) fn create_wallet(id: &str) -> Result<Self, WalletError> {
        let path = Self::wallet_path(id)?;
        Self::directory_builder()
            .recursive(true)
            .create("wallets")
            .map_err(storage_error)?;
        // Create the UUID directory exclusively: even a collision cannot overwrite an account.
        Self::directory_builder()
            .create(&path)
            .map_err(storage_error)?;
        Self::open_db(&path)
    }

    pub(crate) fn open_existing(path: &Path) -> Result<Self, WalletError> {
        if !std::fs::symlink_metadata(path.join("db"))
            .map_err(storage_error)?
            .is_file()
        {
            return Err(storage_error("Wallet database missing"));
        }
        Self::open_db(path)
    }

    fn open_db(path: &Path) -> Result<Self, WalletError> {
        if !std::fs::symlink_metadata(path)
            .map_err(storage_error)?
            .is_dir()
        {
            return Err(storage_error("Wallet path must be a directory"));
        }
        Ok(Self {
            db: sled::open(path).map_err(storage_error)?,
        })
    }

    fn account_bytes(accounts: &HashMap<Address, WalletAccount>) -> Result<Vec<u8>, WalletError> {
        serde_json::to_vec(&accounts.values().collect::<Vec<_>>()).map_err(storage_error)
    }

    // Commit the cursor, encrypted keystore and its matching accounts together.
    pub(crate) async fn save_snapshot(
        &self,
        data: &WalletData,
        accounts: &HashMap<Address, WalletAccount>,
    ) -> Result<(), WalletError> {
        let mut batch = sled::Batch::default();
        batch.insert(WALLET, serde_json::to_vec(data).map_err(storage_error)?);
        batch.insert(ACCOUNTS, Self::account_bytes(accounts)?);
        self.db.apply_batch(batch).map_err(storage_error)?;
        self.db.flush_async().await.map_err(storage_error)?;
        Ok(())
    }

    fn read(&self, key: &[u8]) -> Result<sled::IVec, WalletError> {
        self.db
            .get(key)
            .map_err(storage_error)?
            .ok_or_else(|| storage_error("Wallet record missing"))
    }
}

#[async_trait]
impl Storage for WalletStorage {
    async fn save_wallet_data(&self, data: &WalletData) -> Result<(), WalletError> {
        self.db
            .insert(WALLET, serde_json::to_vec(data).map_err(storage_error)?)
            .map_err(storage_error)?;
        self.db.flush_async().await.map_err(storage_error)?;
        Ok(())
    }

    async fn load_wallet_data(&self) -> Result<WalletData, WalletError> {
        serde_json::from_slice(&self.read(WALLET)?).map_err(storage_error)
    }

    async fn save_accounts(
        &self,
        accounts: &HashMap<Address, WalletAccount>,
    ) -> Result<(), WalletError> {
        self.db
            .insert(ACCOUNTS, Self::account_bytes(accounts)?)
            .map_err(storage_error)?;
        self.db.flush_async().await.map_err(storage_error)?;
        Ok(())
    }

    async fn load_accounts(&self) -> Result<HashMap<Address, WalletAccount>, WalletError> {
        let accounts: Vec<WalletAccount> =
            serde_json::from_slice(&self.read(ACCOUNTS)?).map_err(storage_error)?;
        Ok(accounts.into_iter().map(|a| (a.address, a)).collect())
    }

    async fn delete_all(&self) -> Result<(), WalletError> {
        let mut batch = sled::Batch::default();
        batch.remove(WALLET);
        batch.remove(ACCOUNTS);
        self.db.apply_batch(batch).map_err(storage_error)?;
        self.db.flush_async().await.map_err(storage_error)?;
        Ok(())
    }
}
