//! Integration tests for RocksDB persistence in the Blockchain struct.
//!
//! Run with:
//!   cargo test --manifest-path core/blockchain/Cargo.toml --test blockchain_persistence_test 2>&1 | tail -30

use isa_chain_core::{
    Blockchain,
    block::{Block, ConsensusData, StakeData},
    storage::RocksDbStorage,
    types::{Address, Hash, constants},
};

// ────────────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────────────

fn make_empty_block(height: u64, parent_hash: Hash) -> Block {
    Block::new(
        height,
        parent_hash,
        vec![],
        Hash::hash_data(b"state_root"),
        Hash::hash_data(b"receipts_root"),
        Address::ZERO,
        constants::MAX_GAS_PER_BLOCK,
        ConsensusData {
            validator_signatures: vec![],
            stake_data: StakeData {
                total_stake: 0,
                min_stake: constants::VALIDATOR_MIN_STAKE,
                slash_penalties: vec![],
            },
            randomness: Hash::hash_data(b"randomness"),
        },
    )
}

fn open_storage(dir: &tempfile::TempDir) -> RocksDbStorage {
    RocksDbStorage::new(dir.path()).expect("open RocksDB")
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

/// A blockchain created with new_with_storage on a fresh DB should produce a
/// genesis block and persist it immediately.
#[test]
fn test_new_with_storage_creates_genesis() {
    let dir = tempfile::tempdir().unwrap();
    let saved_genesis_hash;

    // ── Phase 1: create and verify in-memory state ─────────────────────────
    {
        let storage = open_storage(&dir);
        let bc = Blockchain::new_with_storage(constants::MAIN_CHAIN_ID, storage)
            .expect("new_with_storage");

        assert_eq!(bc.height, 0);
        assert_ne!(bc.genesis_hash, Hash::ZERO);
        assert_eq!(bc.head, bc.genesis_hash);
        assert!(bc.has_storage());

        // Verify genesis is accessible in the in-memory block index.
        let genesis = bc.get_block(&bc.genesis_hash).expect("genesis in memory");
        assert_eq!(genesis.header.height, 0);

        saved_genesis_hash = bc.genesis_hash;
    } // bc (and its RocksDB handle) is dropped here, releasing the file lock.

    // ── Phase 2: re-open DB and confirm genesis was persisted ─────────────
    {
        let db2 = open_storage(&dir);
        let stored = isa_chain_core::storage::Storage::get_block_by_height(&db2, 0)
            .expect("storage read")
            .expect("genesis in db");
        assert_eq!(stored.hash(), saved_genesis_hash);
    }
}

/// Adding blocks to a persistent blockchain and then re-opening the database
/// should restore the exact chain state.
#[test]
fn test_persist_and_recover() {
    let dir = tempfile::tempdir().unwrap();
    let saved_genesis_hash;
    let saved_block1_hash;
    let saved_block2_hash;

    // ── Phase 1: create chain, add two blocks, drop ───────────────────────
    {
        let storage = open_storage(&dir);
        let mut bc = Blockchain::new_with_storage(constants::MAIN_CHAIN_ID, storage)
            .expect("new_with_storage");

        saved_genesis_hash = bc.genesis_hash;

        let block1 = make_empty_block(1, bc.head);
        saved_block1_hash = block1.hash();
        bc.add_block(block1).expect("add block 1");

        let block2 = make_empty_block(2, bc.head);
        saved_block2_hash = block2.hash();
        bc.add_block(block2).expect("add block 2");

        assert_eq!(bc.height, 2);
        assert_eq!(bc.head, saved_block2_hash);
    }

    // ── Phase 2: re-open from the same path ───────────────────────────────
    {
        let storage = open_storage(&dir);
        let bc = Blockchain::new_with_storage(constants::MAIN_CHAIN_ID, storage)
            .expect("recover from storage");

        assert_eq!(bc.height, 2);
        assert_eq!(bc.head, saved_block2_hash);
        assert_eq!(bc.genesis_hash, saved_genesis_hash);

        // All three blocks must be accessible in-memory after recovery.
        assert!(bc.get_block(&saved_genesis_hash).is_some());
        assert!(bc.get_block(&saved_block1_hash).is_some());
        assert!(bc.get_block(&saved_block2_hash).is_some());
    }
}

/// Blockchain::new() must remain fully in-memory and unaffected by storage
/// changes introduced by this issue.
#[test]
fn test_in_memory_still_works() {
    let mut bc = Blockchain::new(constants::MAIN_CHAIN_ID);

    assert!(!bc.has_storage());
    assert_eq!(bc.height, 0);

    let block1 = make_empty_block(1, bc.head);
    bc.add_block(block1).expect("add block");
    assert_eq!(bc.height, 1);

    let block2 = make_empty_block(2, bc.head);
    bc.add_block(block2).expect("add block 2");
    assert_eq!(bc.height, 2);
}

/// Account state (balances) written via mint() must survive a drop-reopen cycle.
#[test]
fn test_account_state_persists() {
    let dir = tempfile::tempdir().unwrap();
    let alice = Address::from([0xAA_u8; 20]);

    // ── Write ─────────────────────────────────────────────────────────────
    {
        let storage = open_storage(&dir);
        let mut bc = Blockchain::new_with_storage(constants::MAIN_CHAIN_ID, storage)
            .expect("new_with_storage");

        bc.mint(alice, 42_000);
        assert_eq!(bc.get_balance(&alice), 42_000);

        // Add a block to flush account state via add_block persistence path too.
        let block1 = make_empty_block(1, bc.head);
        bc.add_block(block1).expect("add block");
    }

    // ── Verify account survived in RocksDB ────────────────────────────────
    {
        let db = open_storage(&dir);
        let acc = isa_chain_core::storage::Storage::get_account(&db, &alice)
            .expect("storage read")
            .expect("alice should be persisted");
        assert_eq!(acc.balance, 42_000);
    }
}
