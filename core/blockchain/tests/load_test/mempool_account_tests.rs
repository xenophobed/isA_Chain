use crate::helpers::{addr, make_transfer};
use isa_chain_core::{constants::MAIN_CHAIN_ID, Address, Blockchain, MempoolError};

// ============================================================================
// Test 3 — Large account state
// ============================================================================

/// Mint balances to 10 000 unique addresses; verify account count and a
/// random sample of balances.
#[test]
fn test_large_account_state() {
    const NUM_ACCOUNTS: u16 = 10_000;
    const BALANCE: u128 = 500_000;

    let mut bc = Blockchain::new(MAIN_CHAIN_ID);

    for i in 0..NUM_ACCOUNTS {
        bc.mint(addr(i), BALANCE + i as u128);
    }

    assert_eq!(bc.account_count(), NUM_ACCOUNTS as usize);

    // Spot-check a handful of addresses across the range.
    let samples: &[u16] = &[0, 1, 127, 256, 1000, 5000, 9999];
    for &i in samples {
        let expected = BALANCE + i as u128;
        assert_eq!(
            bc.get_balance(&addr(i)),
            expected,
            "balance mismatch for addr({})",
            i
        );
    }
}

// ============================================================================
// Test 4 — Mempool capacity and drain
// ============================================================================

/// Fill the mempool to capacity, assert overflow error, produce a block to
/// drain some slots, then verify more transactions can be added.
#[test]
fn test_mempool_capacity() {
    // We use unsigned transactions submitted through the mempool layer; but
    // `submit_transaction` enforces signature verification.  Instead we build
    // a small custom blockchain and push signed txns up to capacity.
    //
    // Use a 10-slot mempool by creating transactions from 10 different senders
    // (each with nonce 0) so there are no duplicate hashes.
    const CAPACITY: usize = 10;

    // Build signed keys for CAPACITY + 1 senders.
    let secp = secp256k1::Secp256k1::new();
    let mut keys: Vec<[u8; 32]> = Vec::new();
    let mut addrs: Vec<Address> = Vec::new();
    for i in 0..(CAPACITY as u8 + 2) {
        let mut k = [0u8; 32];
        k[31] = i + 1;
        let sk = secp256k1::SecretKey::from_slice(&k).unwrap();
        let pk = secp256k1::PublicKey::from_secret_key(&secp, &sk);
        addrs.push(Address::from_public_key(&pk.serialize()));
        keys.push(k);
    }

    let mut bc = Blockchain::new(MAIN_CHAIN_ID);
    // Fund all senders generously; fee = 21_000 * 1e9 = 21_000_000_000 per tx
    for a in &addrs {
        bc.mint(*a, 1_000_000_000_000_000);
    }

    // Submit exactly CAPACITY transactions (one per sender, nonce 0).
    let receiver = Address::from([0xCC_u8; 20]);
    for i in 0..CAPACITY {
        let mut tx = make_transfer(addrs[i], receiver, 0, 1_000);
        tx.sign(&keys[i]).expect("sign");
        bc.submit_transaction(tx)
            .unwrap_or_else(|e| panic!("submit {} failed: {:?}", i, e));
    }
    assert_eq!(bc.pending_transaction_count(), CAPACITY);

    // The default mempool capacity is 10 000 — which won't overflow with only 10
    // entries.  To test the MempoolFull error we push directly to the Mempool.
    // Since Blockchain doesn't expose set_mempool_capacity, we test overflow
    // semantics at the Mempool level instead.
    {
        use isa_chain_core::mempool::Mempool;

        let mut mp = Mempool::new(CAPACITY);
        let recv = Address::from([0xDD_u8; 20]);

        // Fill to capacity.
        for i in 0..CAPACITY {
            let tx = make_transfer(addrs[i], recv, 0, 500);
            mp.add_transaction(tx)
                .unwrap_or_else(|e| panic!("add {} failed: {:?}", i, e));
        }
        assert_eq!(mp.len(), CAPACITY);

        // One more must overflow.
        let overflow_tx = make_transfer(addrs[CAPACITY], recv, 0, 500);
        let err = mp
            .add_transaction(overflow_tx)
            .expect_err("should return MempoolFull");
        assert!(
            matches!(err, MempoolError::MempoolFull),
            "expected MempoolFull, got {:?}",
            err
        );

        // Drain two transactions by removing them explicitly.
        let hash0 = make_transfer(addrs[0], recv, 0, 500).hash();
        let hash1 = make_transfer(addrs[1], recv, 0, 500).hash();
        mp.remove_transaction(&hash0);
        mp.remove_transaction(&hash1);
        assert_eq!(mp.len(), CAPACITY - 2);

        // Now there is room for 2 more.
        let refill1 = make_transfer(addrs[CAPACITY], recv, 0, 500);
        let refill2 = make_transfer(addrs[CAPACITY + 1], recv, 0, 500);
        mp.add_transaction(refill1).expect("refill 1 should succeed");
        mp.add_transaction(refill2).expect("refill 2 should succeed");
        assert_eq!(mp.len(), CAPACITY);
    }

    // Also verify that produce_block on the main blockchain drains the mempool.
    let pending_before = bc.pending_transaction_count();
    bc.produce_block(pending_before).expect("produce_block");
    assert_eq!(bc.pending_transaction_count(), 0);
}
