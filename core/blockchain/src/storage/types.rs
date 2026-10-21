use crate::account::Account;
use crate::block::Block;
use crate::error::*;
use crate::transaction::Transaction;
use crate::types::*;

/// Column family names for RocksDB
pub const CF_BLOCKS: &str = "blocks";
pub const CF_BLOCK_BY_HEIGHT: &str = "block_by_height";
pub const CF_TRANSACTIONS: &str = "transactions";
pub const CF_ACCOUNTS: &str = "accounts";
pub const CF_METADATA: &str = "metadata";

/// Key used to store the latest block height in the metadata column family
pub const META_LATEST_HEIGHT: &[u8] = b"latest_height";

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
