use super::*;
use crate::block::{Block, ConsensusData, StakeData};
use crate::types::*;

fn make_blockchain() -> Blockchain {
    Blockchain::new(constants::MAIN_CHAIN_ID)
}

fn make_empty_block(height: BlockHeight, parent_hash: Hash) -> Block {
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

#[test]
fn test_new_blockchain() {
    let bc = make_blockchain();
    assert_eq!(bc.height, 0);
    assert_ne!(bc.genesis_hash, Hash::ZERO);
    assert_eq!(bc.head, bc.genesis_hash);
}

#[test]
fn test_get_balance_empty() {
    let bc = make_blockchain();
    let unknown = Address::from([0xABu8; 20]);
    assert_eq!(bc.get_balance(&unknown), 0);
}

#[test]
fn test_mint_credits_balance() {
    let mut bc = make_blockchain();
    let addr = Address::from([1u8; 20]);
    bc.mint(addr, 5_000);
    assert_eq!(bc.get_balance(&addr), 5_000);
}

#[test]
fn test_add_block() {
    let mut bc = make_blockchain();
    let parent_hash = bc.head;
    let block = make_empty_block(1, parent_hash);
    assert!(bc.add_block(block).is_ok());
    assert_eq!(bc.height, 1);
}

#[test]
fn test_add_block_wrong_height() {
    let mut bc = make_blockchain();
    let parent_hash = bc.head;
    // height 2 is wrong — chain is at 0, expects 1
    let block = make_empty_block(2, parent_hash);
    let result = bc.add_block(block);
    assert!(result.is_err());
}

#[test]
fn test_add_block_wrong_parent() {
    let mut bc = make_blockchain();
    // use a wrong parent hash
    let wrong_parent = Hash::hash_data(b"not_the_real_parent");
    let block = make_empty_block(1, wrong_parent);
    let result = bc.add_block(block);
    assert!(result.is_err());
}

#[test]
fn test_token_state_accessible() {
    let bc = make_blockchain();
    let supply = bc.token_state().get_total_supply();
    assert_eq!(supply, constants::INITIAL_SUPPLY);
}

#[test]
fn test_treasury_accessible() {
    let bc = make_blockchain();
    // Verify the accessor compiles and returns the struct without panic
    let _treasury = bc.treasury();
}

#[test]
fn test_staking_vault_accessible() {
    let bc = make_blockchain();
    let _vault = bc.staking_vault();
}

#[test]
fn test_produce_block() {
    let mut bc = make_blockchain();
    let result = bc.produce_block(100);
    assert!(result.is_ok());
    assert_eq!(bc.height, 1);
}

/// Settlement transaction: gross_amount deducted from user, net credited to
/// provider, fee credited to treasury (Address::ZERO in tests).
#[test]
fn test_execute_settlement_transaction() {
    use crate::transaction::{Transaction, TransactionData};

    let user_addr    = Address::from([0x01u8; 20]);
    let provider_addr = Address::from([0x02u8; 20]);
    let gross: Amount = 10_000;
    let fee: Amount   = 250;
    let net: Amount   = gross - fee; // 9_750

    let mut bc = make_blockchain();
    bc.mint(user_addr, gross);
    assert_eq!(bc.get_balance(&user_addr), gross);

    // Seed a nonce-0 account so the entry exists in the map.
    bc.accounts.entry(user_addr)
        .and_modify(|acc| acc.nonce = 0);

    let tx = Transaction::new(
        user_addr,
        0,
        TransactionData::Settlement {
            user: user_addr,
            provider: provider_addr,
            gross_amount: gross,
            fee_amount: fee,
        },
        21_000,
        constants::BASE_GAS_PRICE,
        constants::MAIN_CHAIN_ID,
    );

    assert!(bc.execute_transaction(&tx).is_ok());

    // User balance fully drained.
    assert_eq!(bc.get_balance(&user_addr), 0);
    // Provider receives net amount.
    assert_eq!(bc.get_balance(&provider_addr), net);
    // Fee lands at treasury address (Address::ZERO).
    assert_eq!(bc.get_balance(&Address::ZERO), fee);
}
