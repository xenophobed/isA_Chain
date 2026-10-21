use crate::helpers::{addr, make_transfer};
use isa_chain_core::{constants::MAIN_CHAIN_ID, Address, Blockchain};

// ============================================================================
// Test 1 — High transaction throughput
// ============================================================================

/// Stress test: 10 funded addresses each submit 100 transfers → 1 000 txns total.
/// Blocks are produced until the mempool is drained.
///
/// Assertions:
/// - All 1 000 transactions are processed (total height reflects them).
/// - Every sender still has a consistent (non-overflowed) balance at the end.
#[test]
fn test_high_transaction_throughput() {
    const ADDRS: u16 = 10;
    const TXS_PER_ADDR: u64 = 100;
    // fee = 21_000 gas * 1_000_000_000 gas_price = 21_000_000_000 per tx
    // 100 txns * 21_000_000_000 = 2_100_000_000_000; give 10x headroom
    const INITIAL_BALANCE: u128 = 100_000_000_000_000;
    const TRANSFER_AMOUNT: u128 = 1_000;
    const BLOCK_TX_CAP: usize = 256; // transactions per block

    let mut bc = Blockchain::new(MAIN_CHAIN_ID);

    // Fund all senders.
    for i in 0..ADDRS {
        bc.mint(addr(i), INITIAL_BALANCE);
    }

    // Use signed transactions so submit_transaction accepts them.
    // Derive private keys deterministically: key[0] = i as u8, rest zeros.
    // The `from` address must match the key, so we derive address from key.
    let secp = secp256k1::Secp256k1::new();
    let mut addresses: Vec<Address> = Vec::new();
    let mut private_keys: Vec<[u8; 32]> = Vec::new();

    for i in 0..ADDRS {
        let mut key_bytes = [0u8; 32];
        // Use a non-zero seed: index + 1 to avoid the all-zeros invalid key.
        key_bytes[31] = (i as u8).wrapping_add(1);
        key_bytes[30] = (i >> 8) as u8;
        let secret_key = secp256k1::SecretKey::from_slice(&key_bytes).unwrap();
        let pub_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
        let derived_addr = Address::from_public_key(&pub_key.serialize());
        addresses.push(derived_addr);
        private_keys.push(key_bytes);
    }

    // Re-fund derived addresses.
    let mut bc = Blockchain::new(MAIN_CHAIN_ID);
    for a in &addresses {
        bc.mint(*a, INITIAL_BALANCE);
    }

    // Use a fixed external receiver that is NOT in the `addresses` set.
    // This ensures sender balances strictly decrease and receiver balance increases.
    let external_receiver = Address::from([0xEF_u8; 20]);

    // Submit and commit transactions in nonce-ordered rounds.
    //
    // `submit_transaction` checks the *committed* nonce — not the pending
    // mempool nonce — so we must commit nonce N before submitting nonce N+1
    // for the same sender.  We submit one nonce-level at a time (10
    // transactions, one per sender) and produce a block per round.
    let mut total_processed = 0usize;
    for nonce in 0..TXS_PER_ADDR {
        for i in 0..ADDRS as usize {
            let from = addresses[i];
            let mut tx = make_transfer(from, external_receiver, nonce, TRANSFER_AMOUNT);
            tx.sign(&private_keys[i]).expect("sign should succeed");
            bc.submit_transaction(tx).expect("submit_transaction failed");
        }
        // Commit this round's transactions into a block.
        bc.produce_block(BLOCK_TX_CAP).expect("produce_block failed");
        total_processed += ADDRS as usize;
    }

    assert_eq!(total_processed, (ADDRS as usize) * (TXS_PER_ADDR as usize));
    assert_eq!(bc.pending_transaction_count(), 0);
    assert_eq!(bc.get_height(), TXS_PER_ADDR, "one block per nonce round");

    // Each sender has sent TXS_PER_ADDR * TRANSFER_AMOUNT ISA out.
    // execute_transaction deducts only the transfer amount (fees are checked
    // at submission but not yet deducted in the current implementation).
    let expected_debit = TXS_PER_ADDR as u128 * TRANSFER_AMOUNT;
    for a in &addresses {
        let balance = bc.get_balance(a);
        assert_eq!(
            balance,
            INITIAL_BALANCE - expected_debit,
            "sender {:?} balance mismatch (expected {}, got {})",
            a,
            INITIAL_BALANCE - expected_debit,
            balance
        );
    }

    // The external receiver collected all transfers.
    let expected_receiver_balance = (ADDRS as u128) * (TXS_PER_ADDR as u128) * TRANSFER_AMOUNT;
    assert_eq!(
        bc.get_balance(&external_receiver),
        expected_receiver_balance,
        "receiver balance mismatch"
    );
}

// ============================================================================
// Test 2 — Block production performance
// ============================================================================

/// Produce 100 consecutive blocks and verify the chain is correctly linked.
#[test]
fn test_block_production_performance() {
    const NUM_BLOCKS: u64 = 100;

    let mut bc = Blockchain::new(MAIN_CHAIN_ID);

    // Fund a sender so some blocks can contain real transactions.
    let secp = secp256k1::Secp256k1::new();
    let mut key_bytes = [0u8; 32];
    key_bytes[31] = 7;
    let secret_key = secp256k1::SecretKey::from_slice(&key_bytes).unwrap();
    let pub_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
    let sender = Address::from_public_key(&pub_key.serialize());
    let receiver = Address::from([0xBB_u8; 20]);
    // fee = 21_000 * 1_000_000_000 = 21_000_000_000 per tx; 100 txns, give ample headroom
    bc.mint(sender, 10_000_000_000_000_000);

    // Produce NUM_BLOCKS blocks; submit one transaction before each.
    for block_i in 0..NUM_BLOCKS {
        let mut tx = make_transfer(sender, receiver, block_i, 100);
        tx.sign(&key_bytes).expect("sign");
        bc.submit_transaction(tx).expect("submit");

        let _hash = bc.produce_block(10).expect("produce_block");
    }

    assert_eq!(bc.get_height(), NUM_BLOCKS);

    // Verify parent linkage for every block.
    for height in 1..=NUM_BLOCKS {
        let block = bc
            .get_block_by_height(height)
            .unwrap_or_else(|| panic!("block at height {} not found", height));
        let parent = bc
            .get_block_by_height(height - 1)
            .unwrap_or_else(|| panic!("parent block at height {} not found", height - 1));
        assert_eq!(
            block.header.parent_hash,
            parent.hash(),
            "block {} parent hash mismatch",
            height
        );
    }
}
