use super::types::*;
use crate::account::Account;
use crate::block::Block;
use crate::error::*;
use crate::transaction::Transaction;
use crate::types::*;
use std::collections::HashMap;

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
