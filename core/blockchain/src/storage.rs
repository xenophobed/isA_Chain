use crate::types::*;
use crate::block::Block;
use crate::transaction::Transaction;
use crate::account::Account;
use crate::error::*;
use std::collections::HashMap;
use std::path::Path;

/// Column family names for RocksDB
const CF_BLOCKS: &str = "blocks";
const CF_BLOCK_BY_HEIGHT: &str = "block_by_height";
const CF_TRANSACTIONS: &str = "transactions";
const CF_ACCOUNTS: &str = "accounts";
const CF_METADATA: &str = "metadata";

/// Key used to store the latest block height in the metadata column family
const META_LATEST_HEIGHT: &[u8] = b"latest_height";

/// Storage interface trait
pub trait Storage {
    fn get_block(&self, hash: &Hash) -> Result<Option<Block>, StorageError>;
    fn put_block(&mut self, block: Block) -> Result<(), StorageError>;

    fn get_block_by_height(&self, height: BlockHeight) -> Result<Option<Block>, StorageError>;
    fn get_latest_height(&self) -> Result<Option<BlockHeight>, StorageError>;

    fn get_transaction(&self, hash: &Hash) -> Result<Option<Transaction>, StorageError>;
    fn put_transaction(&mut self, tx: Transaction) -> Result<(), StorageError>;

    fn get_account(&self, address: &Address) -> Result<Option<Account>, StorageError>;
    fn put_account(&mut self, address: Address, account: Account) -> Result<(), StorageError>;

    fn put_metadata(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError>;
    fn get_metadata(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError>;
}

// ---------------------------------------------------------------------------
// RocksDB Storage
// ---------------------------------------------------------------------------

/// RocksDB-backed persistent storage implementation.
pub struct RocksDbStorage {
    db: rocksdb::DB,
}

fn column_families() -> Vec<&'static str> {
    vec![CF_BLOCKS, CF_BLOCK_BY_HEIGHT, CF_TRANSACTIONS, CF_ACCOUNTS, CF_METADATA]
}

impl RocksDbStorage {
    /// Open or create a RocksDB database at the given path.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let cf_descriptors: Vec<rocksdb::ColumnFamilyDescriptor> = column_families()
            .into_iter()
            .map(|name| {
                rocksdb::ColumnFamilyDescriptor::new(name, rocksdb::Options::default())
            })
            .collect();

        let db = rocksdb::DB::open_cf_descriptors(&opts, path, cf_descriptors)?;
        Ok(RocksDbStorage { db })
    }

    fn cf(&self, name: &str) -> Result<&rocksdb::ColumnFamily, StorageError> {
        self.db
            .cf_handle(name)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(name.to_string()))
    }
}

impl Storage for RocksDbStorage {
    fn get_block(&self, hash: &Hash) -> Result<Option<Block>, StorageError> {
        let cf = self.cf(CF_BLOCKS)?;
        match self.db.get_cf(cf, hash.as_bytes())? {
            Some(bytes) => Ok(Some(bincode::deserialize(&bytes)?)),
            None => Ok(None),
        }
    }

    fn put_block(&mut self, block: Block) -> Result<(), StorageError> {
        let hash = block.hash();
        let height = block.header.height;
        let encoded = bincode::serialize(&block)?;

        // Store block by hash
        let cf_blocks = self.cf(CF_BLOCKS)?;
        self.db.put_cf(cf_blocks, hash.as_bytes(), &encoded)?;

        // Index block by height
        let cf_height = self.cf(CF_BLOCK_BY_HEIGHT)?;
        self.db.put_cf(cf_height, height.to_be_bytes(), hash.as_bytes())?;

        // Update latest height if this block is higher
        let should_update = match self.get_latest_height()? {
            Some(current) => height > current,
            None => true,
        };
        if should_update {
            let cf_meta = self.cf(CF_METADATA)?;
            self.db.put_cf(cf_meta, META_LATEST_HEIGHT, height.to_be_bytes())?;
        }

        Ok(())
    }

    fn get_block_by_height(&self, height: BlockHeight) -> Result<Option<Block>, StorageError> {
        let cf_height = self.cf(CF_BLOCK_BY_HEIGHT)?;
        match self.db.get_cf(cf_height, height.to_be_bytes())? {
            Some(hash_bytes) => {
                let hash = Hash::from_bytes(&hash_bytes)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                self.get_block(&hash)
            }
            None => Ok(None),
        }
    }

    fn get_latest_height(&self) -> Result<Option<BlockHeight>, StorageError> {
        let cf_meta = self.cf(CF_METADATA)?;
        match self.db.get_cf(cf_meta, META_LATEST_HEIGHT)? {
            Some(bytes) => {
                let arr: [u8; 8] = bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| StorageError::Serialization("invalid height bytes".to_string()))?;
                Ok(Some(u64::from_be_bytes(arr)))
            }
            None => Ok(None),
        }
    }

    fn get_transaction(&self, hash: &Hash) -> Result<Option<Transaction>, StorageError> {
        let cf = self.cf(CF_TRANSACTIONS)?;
        match self.db.get_cf(cf, hash.as_bytes())? {
            Some(bytes) => Ok(Some(bincode::deserialize(&bytes)?)),
            None => Ok(None),
        }
    }

    fn put_transaction(&mut self, tx: Transaction) -> Result<(), StorageError> {
        let hash = tx.hash();
        let encoded = bincode::serialize(&tx)?;
        let cf = self.cf(CF_TRANSACTIONS)?;
        self.db.put_cf(cf, hash.as_bytes(), &encoded)?;
        Ok(())
    }

    fn get_account(&self, address: &Address) -> Result<Option<Account>, StorageError> {
        let cf = self.cf(CF_ACCOUNTS)?;
        match self.db.get_cf(cf, address.as_bytes())? {
            Some(bytes) => Ok(Some(bincode::deserialize(&bytes)?)),
            None => Ok(None),
        }
    }

    fn put_account(&mut self, address: Address, account: Account) -> Result<(), StorageError> {
        let encoded = bincode::serialize(&account)?;
        let cf = self.cf(CF_ACCOUNTS)?;
        self.db.put_cf(cf, address.as_bytes(), &encoded)?;
        Ok(())
    }

    fn put_metadata(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        let cf = self.cf(CF_METADATA)?;
        self.db.put_cf(cf, key, value)?;
        Ok(())
    }

    fn get_metadata(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        let cf = self.cf(CF_METADATA)?;
        Ok(self.db.get_cf(cf, key)?)
    }
}

// ---------------------------------------------------------------------------
// In-Memory Storage (for testing)
// ---------------------------------------------------------------------------

/// HashMap-backed in-memory storage implementation, useful for testing.
pub struct InMemoryStorage {
    blocks: HashMap<Hash, Vec<u8>>,
    block_by_height: HashMap<BlockHeight, Hash>,
    transactions: HashMap<Hash, Vec<u8>>,
    accounts: HashMap<Address, Vec<u8>>,
    metadata: HashMap<Vec<u8>, Vec<u8>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        InMemoryStorage {
            blocks: HashMap::new(),
            block_by_height: HashMap::new(),
            transactions: HashMap::new(),
            accounts: HashMap::new(),
            metadata: HashMap::new(),
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage for InMemoryStorage {
    fn get_block(&self, hash: &Hash) -> Result<Option<Block>, StorageError> {
        match self.blocks.get(hash) {
            Some(bytes) => Ok(Some(bincode::deserialize(bytes)?)),
            None => Ok(None),
        }
    }

    fn put_block(&mut self, block: Block) -> Result<(), StorageError> {
        let hash = block.hash();
        let height = block.header.height;
        let encoded = bincode::serialize(&block)?;

        self.blocks.insert(hash, encoded);
        self.block_by_height.insert(height, hash);

        // Update latest height
        let should_update = match self.get_latest_height()? {
            Some(current) => height > current,
            None => true,
        };
        if should_update {
            self.metadata
                .insert(META_LATEST_HEIGHT.to_vec(), height.to_be_bytes().to_vec());
        }

        Ok(())
    }

    fn get_block_by_height(&self, height: BlockHeight) -> Result<Option<Block>, StorageError> {
        match self.block_by_height.get(&height) {
            Some(hash) => self.get_block(hash),
            None => Ok(None),
        }
    }

    fn get_latest_height(&self) -> Result<Option<BlockHeight>, StorageError> {
        match self.metadata.get(META_LATEST_HEIGHT) {
            Some(bytes) => {
                let arr: [u8; 8] = bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| StorageError::Serialization("invalid height bytes".to_string()))?;
                Ok(Some(u64::from_be_bytes(arr)))
            }
            None => Ok(None),
        }
    }

    fn get_transaction(&self, hash: &Hash) -> Result<Option<Transaction>, StorageError> {
        match self.transactions.get(hash) {
            Some(bytes) => Ok(Some(bincode::deserialize(bytes)?)),
            None => Ok(None),
        }
    }

    fn put_transaction(&mut self, tx: Transaction) -> Result<(), StorageError> {
        let hash = tx.hash();
        let encoded = bincode::serialize(&tx)?;
        self.transactions.insert(hash, encoded);
        Ok(())
    }

    fn get_account(&self, address: &Address) -> Result<Option<Account>, StorageError> {
        match self.accounts.get(address) {
            Some(bytes) => Ok(Some(bincode::deserialize(bytes)?)),
            None => Ok(None),
        }
    }

    fn put_account(&mut self, address: Address, account: Account) -> Result<(), StorageError> {
        let encoded = bincode::serialize(&account)?;
        self.accounts.insert(address, encoded);
        Ok(())
    }

    fn put_metadata(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        self.metadata.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn get_metadata(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(self.metadata.get(key).cloned())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{ConsensusData, StakeData};
    use crate::transaction::{TransactionData, Transaction};
    use crate::types::constants;

    /// Helper: create a simple test block at a given height with a given parent hash.
    fn make_test_block(height: BlockHeight, parent_hash: Hash) -> Block {
        let consensus_data = ConsensusData {
            validator_signatures: vec![],
            stake_data: StakeData {
                total_stake: 0,
                min_stake: constants::VALIDATOR_MIN_STAKE,
                slash_penalties: vec![],
            },
            randomness: Hash::hash_data(format!("rand_{height}").as_bytes()),
        };

        Block::new(
            height,
            parent_hash,
            vec![],
            Hash::hash_data(format!("state_{height}").as_bytes()),
            Hash::ZERO,
            Address::from([1u8; 20]),
            constants::MAX_GAS_PER_BLOCK,
            consensus_data,
        )
    }

    /// Helper: create a simple unsigned test transaction.
    fn make_test_tx(nonce: u64) -> Transaction {
        Transaction::new(
            Address::from([1u8; 20]),
            nonce,
            TransactionData::Transfer {
                to: Address::from([2u8; 20]),
                amount: 1000,
                data: vec![],
            },
            21000,
            constants::BASE_GAS_PRICE,
            constants::MAIN_CHAIN_ID,
        )
    }

    // ============================
    // Generic test helpers — run against any Storage impl
    // ============================

    fn test_put_get_block(storage: &mut dyn Storage) {
        let block = make_test_block(0, Hash::ZERO);
        let hash = block.hash();

        storage.put_block(block.clone()).unwrap();

        let retrieved = storage.get_block(&hash).unwrap().expect("block should exist");
        assert_eq!(retrieved.hash(), hash);
        assert_eq!(retrieved.header.height, 0);
    }

    fn test_block_by_height(storage: &mut dyn Storage) {
        let block0 = make_test_block(0, Hash::ZERO);
        let block1 = make_test_block(1, block0.hash());

        storage.put_block(block0.clone()).unwrap();
        storage.put_block(block1.clone()).unwrap();

        let b0 = storage.get_block_by_height(0).unwrap().expect("height 0");
        assert_eq!(b0.hash(), block0.hash());

        let b1 = storage.get_block_by_height(1).unwrap().expect("height 1");
        assert_eq!(b1.hash(), block1.hash());

        assert!(storage.get_block_by_height(99).unwrap().is_none());
    }

    fn test_latest_height(storage: &mut dyn Storage) {
        assert!(storage.get_latest_height().unwrap().is_none());

        let block0 = make_test_block(0, Hash::ZERO);
        storage.put_block(block0.clone()).unwrap();
        assert_eq!(storage.get_latest_height().unwrap(), Some(0));

        let block1 = make_test_block(1, block0.hash());
        storage.put_block(block1).unwrap();
        assert_eq!(storage.get_latest_height().unwrap(), Some(1));
    }

    fn test_put_get_transaction(storage: &mut dyn Storage) {
        let tx = make_test_tx(0);
        let hash = tx.hash();

        storage.put_transaction(tx).unwrap();

        let retrieved = storage.get_transaction(&hash).unwrap().expect("tx should exist");
        assert_eq!(retrieved.hash(), hash);
        assert_eq!(retrieved.nonce, 0);

        assert!(storage.get_transaction(&Hash::ZERO).unwrap().is_none());
    }

    fn test_put_get_account(storage: &mut dyn Storage) {
        let address = Address::from([10u8; 20]);
        let account = Account::new_external(5000);

        storage.put_account(address, account.clone()).unwrap();

        let retrieved = storage.get_account(&address).unwrap().expect("account should exist");
        assert_eq!(retrieved.balance, 5000);
        assert_eq!(retrieved.nonce, 0);

        assert!(storage.get_account(&Address::ZERO).unwrap().is_none());
    }

    fn test_account_update(storage: &mut dyn Storage) {
        let address = Address::from([20u8; 20]);
        let mut account = Account::new_external(1000);

        storage.put_account(address, account.clone()).unwrap();

        // Update the account
        account.balance = 2000;
        account.nonce = 5;
        storage.put_account(address, account).unwrap();

        let retrieved = storage.get_account(&address).unwrap().expect("account should exist");
        assert_eq!(retrieved.balance, 2000);
        assert_eq!(retrieved.nonce, 5);
    }

    fn test_metadata(storage: &mut dyn Storage) {
        assert!(storage.get_metadata(b"my_key").unwrap().is_none());

        storage.put_metadata(b"my_key", b"my_value").unwrap();

        let val = storage.get_metadata(b"my_key").unwrap().expect("metadata should exist");
        assert_eq!(val, b"my_value");

        // Overwrite
        storage.put_metadata(b"my_key", b"updated").unwrap();
        let val = storage.get_metadata(b"my_key").unwrap().expect("metadata should exist");
        assert_eq!(val, b"updated");
    }

    // ============================
    // InMemoryStorage unit tests
    // ============================

    #[test]
    fn inmemory_put_get_block() {
        let mut s = InMemoryStorage::new();
        test_put_get_block(&mut s);
    }

    #[test]
    fn inmemory_block_by_height() {
        let mut s = InMemoryStorage::new();
        test_block_by_height(&mut s);
    }

    #[test]
    fn inmemory_latest_height() {
        let mut s = InMemoryStorage::new();
        test_latest_height(&mut s);
    }

    #[test]
    fn inmemory_put_get_transaction() {
        let mut s = InMemoryStorage::new();
        test_put_get_transaction(&mut s);
    }

    #[test]
    fn inmemory_put_get_account() {
        let mut s = InMemoryStorage::new();
        test_put_get_account(&mut s);
    }

    #[test]
    fn inmemory_account_update() {
        let mut s = InMemoryStorage::new();
        test_account_update(&mut s);
    }

    #[test]
    fn inmemory_metadata() {
        let mut s = InMemoryStorage::new();
        test_metadata(&mut s);
    }

    // ============================
    // RocksDbStorage integration tests
    // ============================

    fn temp_rocks() -> (tempfile::TempDir, RocksDbStorage) {
        let dir = tempfile::tempdir().unwrap();
        let storage = RocksDbStorage::new(dir.path()).unwrap();
        (dir, storage)
    }

    #[test]
    fn rocks_put_get_block() {
        let (_dir, mut s) = temp_rocks();
        test_put_get_block(&mut s);
    }

    #[test]
    fn rocks_block_by_height() {
        let (_dir, mut s) = temp_rocks();
        test_block_by_height(&mut s);
    }

    #[test]
    fn rocks_latest_height() {
        let (_dir, mut s) = temp_rocks();
        test_latest_height(&mut s);
    }

    #[test]
    fn rocks_put_get_transaction() {
        let (_dir, mut s) = temp_rocks();
        test_put_get_transaction(&mut s);
    }

    #[test]
    fn rocks_put_get_account() {
        let (_dir, mut s) = temp_rocks();
        test_put_get_account(&mut s);
    }

    #[test]
    fn rocks_account_update() {
        let (_dir, mut s) = temp_rocks();
        test_account_update(&mut s);
    }

    #[test]
    fn rocks_metadata() {
        let (_dir, mut s) = temp_rocks();
        test_metadata(&mut s);
    }

    #[test]
    fn rocks_persistence_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let block_hash;
        let address = Address::from([42u8; 20]);

        // Write data, then drop the storage
        {
            let mut s = RocksDbStorage::new(dir.path()).unwrap();
            let block = make_test_block(0, Hash::ZERO);
            block_hash = block.hash();
            s.put_block(block).unwrap();

            let account = Account::new_external(9999);
            s.put_account(address, account).unwrap();

            s.put_metadata(b"version", b"1").unwrap();
        }

        // Reopen and verify data persisted
        {
            let s = RocksDbStorage::new(dir.path()).unwrap();

            let block = s.get_block(&block_hash).unwrap().expect("block should persist");
            assert_eq!(block.hash(), block_hash);

            let account = s.get_account(&address).unwrap().expect("account should persist");
            assert_eq!(account.balance, 9999);

            let meta = s.get_metadata(b"version").unwrap().expect("metadata should persist");
            assert_eq!(meta, b"1");

            assert_eq!(s.get_latest_height().unwrap(), Some(0));
        }
    }
}
