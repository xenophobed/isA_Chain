use crate::account::Account;
use crate::block::{Block, ConsensusData, StakeData};
use crate::storage::types::Storage;
use crate::transaction::{Transaction, TransactionData};
use crate::types::constants;
use crate::types::{Address, BlockHeight, Hash};

/// Helper: create a simple test block at a given height with a given parent hash.
pub fn make_test_block(height: BlockHeight, parent_hash: Hash) -> Block {
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
pub fn make_test_tx(nonce: u64) -> Transaction {
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

pub fn test_put_get_block(storage: &mut dyn Storage) {
    let block = make_test_block(0, Hash::ZERO);
    let hash = block.hash();

    storage.put_block(block.clone()).unwrap();

    let retrieved = storage.get_block(&hash).unwrap().expect("block should exist");
    assert_eq!(retrieved.hash(), hash);
    assert_eq!(retrieved.header.height, 0);
}

pub fn test_block_by_height(storage: &mut dyn Storage) {
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

pub fn test_latest_height(storage: &mut dyn Storage) {
    assert!(storage.get_latest_height().unwrap().is_none());

    let block0 = make_test_block(0, Hash::ZERO);
    storage.put_block(block0.clone()).unwrap();
    assert_eq!(storage.get_latest_height().unwrap(), Some(0));

    let block1 = make_test_block(1, block0.hash());
    storage.put_block(block1).unwrap();
    assert_eq!(storage.get_latest_height().unwrap(), Some(1));
}

pub fn test_put_get_transaction(storage: &mut dyn Storage) {
    let tx = make_test_tx(0);
    let hash = tx.hash();

    storage.put_transaction(tx).unwrap();

    let retrieved = storage.get_transaction(&hash).unwrap().expect("tx should exist");
    assert_eq!(retrieved.hash(), hash);
    assert_eq!(retrieved.nonce, 0);

    assert!(storage.get_transaction(&Hash::ZERO).unwrap().is_none());
}

pub fn test_put_get_account(storage: &mut dyn Storage) {
    let address = Address::from([10u8; 20]);
    let account = Account::new_external(5000);

    storage.put_account(address, account.clone()).unwrap();

    let retrieved = storage.get_account(&address).unwrap().expect("account should exist");
    assert_eq!(retrieved.balance, 5000);
    assert_eq!(retrieved.nonce, 0);

    assert!(storage.get_account(&Address::ZERO).unwrap().is_none());
}

pub fn test_account_update(storage: &mut dyn Storage) {
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

pub fn test_metadata(storage: &mut dyn Storage) {
    assert!(storage.get_metadata(b"my_key").unwrap().is_none());

    storage.put_metadata(b"my_key", b"my_value").unwrap();

    let val = storage.get_metadata(b"my_key").unwrap().expect("metadata should exist");
    assert_eq!(val, b"my_value");

    // Overwrite
    storage.put_metadata(b"my_key", b"updated").unwrap();
    let val = storage.get_metadata(b"my_key").unwrap().expect("metadata should exist");
    assert_eq!(val, b"updated");
}
