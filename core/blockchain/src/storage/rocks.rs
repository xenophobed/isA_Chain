use super::types::*;
use crate::account::Account;
use crate::block::Block;
use crate::error::*;
use crate::transaction::Transaction;
use crate::types::*;
use std::path::Path;

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
