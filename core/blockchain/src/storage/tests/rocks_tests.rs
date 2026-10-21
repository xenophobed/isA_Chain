use super::helpers::*;
use crate::account::Account;
use crate::storage::rocks::RocksDbStorage;
use crate::storage::types::Storage;
use crate::types::{Address, Hash};

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
