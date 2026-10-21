use super::helpers::*;
use crate::storage::inmemory::InMemoryStorage;

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
