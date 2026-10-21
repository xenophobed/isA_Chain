use crate::agent_wallet::*;
use crate::types::Address;
use super::{make_agent_id, make_factory, make_owner, DEFAULT_PERIOD, ONE_ISA};

#[test]
fn test_create_wallet() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(1);
    let owner = make_owner(0xAA);

    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .expect("should create wallet");

    let wallet = factory.get_wallet(&addr).expect("wallet should exist");
    assert_eq!(wallet.agent_id, agent_id);
    assert_eq!(wallet.owner, owner);
    assert_eq!(wallet.balance, 0);
    assert_eq!(wallet.spending_limit, ONE_ISA);
    assert_eq!(wallet.period_length, DEFAULT_PERIOD);
    assert_eq!(wallet.status, WalletStatus::Active);
    assert_eq!(wallet.created_at, 0);

    // Deterministic address derivation
    let expected_addr = Address::from_public_key(agent_id.as_bytes());
    assert_eq!(addr, expected_addr);
}

#[test]
fn test_duplicate_agent_fails() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(2);
    let owner = make_owner(0xBB);

    factory
        .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    let result = factory.create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 1, 0, 0);
    assert_eq!(result, Err(AgentWalletError::AgentAlreadyHasWallet(agent_id)));
}

#[test]
fn test_deposit() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(3);
    let owner = make_owner(0xCC);
    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    factory.deposit(&addr, ONE_ISA).unwrap();
    let wallet = factory.get_wallet(&addr).unwrap();
    assert_eq!(wallet.balance, ONE_ISA);

    // Deposit again
    factory.deposit(&addr, 500).unwrap();
    assert_eq!(factory.get_wallet(&addr).unwrap().balance, ONE_ISA + 500);
}

#[test]
fn test_freeze_unfreeze() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(8);
    let owner = make_owner(0x22);
    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    factory.freeze(&addr, &owner).unwrap();
    assert_eq!(
        factory.get_wallet(&addr).unwrap().status,
        WalletStatus::Frozen
    );

    factory.unfreeze(&addr, &owner).unwrap();
    assert_eq!(
        factory.get_wallet(&addr).unwrap().status,
        WalletStatus::Active
    );
}

#[test]
fn test_close_returns_balance() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(9);
    let owner = make_owner(0x33);
    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    factory.deposit(&addr, ONE_ISA).unwrap();
    let returned = factory.close(&addr, &owner).unwrap();

    assert_eq!(returned, ONE_ISA);
    assert_eq!(factory.get_wallet(&addr).unwrap().balance, 0);
    assert_eq!(
        factory.get_wallet(&addr).unwrap().status,
        WalletStatus::Closed
    );

    // Closing again should fail
    let result = factory.close(&addr, &owner);
    assert_eq!(result, Err(AgentWalletError::WalletClosed));
}

#[test]
fn test_set_spending_limit() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(10);
    let owner = make_owner(0x44);
    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    factory.set_spending_limit(&addr, ONE_ISA * 5, &owner).unwrap();
    assert_eq!(
        factory.get_wallet(&addr).unwrap().spending_limit,
        ONE_ISA * 5
    );
}

#[test]
fn test_unauthorized_owner_fails() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(12);
    let owner = make_owner(0x66);
    let intruder = make_owner(0x77);
    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    assert_eq!(
        factory.freeze(&addr, &intruder),
        Err(AgentWalletError::UnauthorizedOwner(intruder))
    );
    assert_eq!(
        factory.close(&addr, &intruder),
        Err(AgentWalletError::UnauthorizedOwner(intruder))
    );
    assert_eq!(
        factory.set_spending_limit(&addr, 0, &intruder),
        Err(AgentWalletError::UnauthorizedOwner(intruder))
    );
}

#[test]
fn test_get_wallets_by_owner() {
    let mut factory = make_factory();
    let owner = make_owner(0x88);

    // Create two wallets for the same owner
    let addr1 = factory
        .create_wallet(make_agent_id(20), owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();
    let addr2 = factory
        .create_wallet(make_agent_id(21), owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    // Different owner — should not appear
    let other_owner = make_owner(0x99);
    factory
        .create_wallet(make_agent_id(22), other_owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    let wallets = factory.get_wallets_by_owner(&owner);
    assert_eq!(wallets.len(), 2);
    let addresses: Vec<Address> = wallets.iter().map(|w| w.address).collect();
    assert!(addresses.contains(&addr1));
    assert!(addresses.contains(&addr2));

    // Owner with no wallets
    let empty_owner = make_owner(0xAB);
    assert!(factory.get_wallets_by_owner(&empty_owner).is_empty());
}
