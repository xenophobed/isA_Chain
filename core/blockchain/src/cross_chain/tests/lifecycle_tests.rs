use super::*;
use crate::cross_chain::types::{BridgeError, ChainType, TransferStatus};
use crate::types::Hash;

#[test]
fn test_add_supported_chain() {
    let mut bridge = CrossChainBridge::new(100, 100, 1_000_000, admin());
    bridge.add_supported_chain(ChainType::Ethereum);
    assert!(bridge.supported_chains.contains(&ChainType::Ethereum));
}

#[test]
fn test_custom_chain_supported() {
    let mut bridge = CrossChainBridge::new(100, 100, 1_000_000, admin());
    bridge.add_supported_chain(ChainType::Custom("Polygon".to_string()));
    assert!(bridge
        .supported_chains
        .contains(&ChainType::Custom("Polygon".to_string())));
}

#[test]
fn test_add_relayer_as_admin() {
    let bridge = setup();
    assert!(bridge.relayers.contains(&relayer()));
}

#[test]
fn test_add_relayer_non_admin_fails() {
    let mut bridge = setup();
    let result = bridge.add_relayer(stranger(), &stranger());
    assert_eq!(result, Err(BridgeError::UnauthorizedAdmin));
}

#[test]
fn test_confirm_transfer_success() {
    let mut bridge = setup();
    let id = bridge
        .initiate_transfer(sender(), valid_recipient(), 10_000, ChainType::Base, 1)
        .unwrap();

    bridge
        .confirm_transfer(&id, "0xdeadbeef".to_string(), &relayer(), 10)
        .unwrap();

    let t = bridge.get_transfer(&id).unwrap();
    assert_eq!(t.status, TransferStatus::Completed);
    assert_eq!(t.external_tx_hash, Some("0xdeadbeef".to_string()));
    assert_eq!(t.completed_at, Some(10));
    // locked should be zero again, transferred = net
    assert_eq!(bridge.get_total_locked(), 0);
    assert_eq!(bridge.total_transferred, 10_000 - bridge.calculate_fee(10_000));
}

#[test]
fn test_confirm_transfer_unauthorized_relayer() {
    let mut bridge = setup();
    let id = bridge
        .initiate_transfer(sender(), valid_recipient(), 1_000, ChainType::Ethereum, 1)
        .unwrap();

    let result =
        bridge.confirm_transfer(&id, "0xdeadbeef".to_string(), &stranger(), 5);
    assert_eq!(result, Err(BridgeError::UnauthorizedRelayer(stranger())));
}

#[test]
fn test_confirm_transfer_not_found() {
    let mut bridge = setup();
    let fake_id = Hash::hash_data(b"nonexistent");
    let result =
        bridge.confirm_transfer(&fake_id, "0xdeadbeef".to_string(), &relayer(), 5);
    assert_eq!(result, Err(BridgeError::TransferNotFound(fake_id)));
}

#[test]
fn test_confirm_transfer_already_completed() {
    let mut bridge = setup();
    let id = bridge
        .initiate_transfer(sender(), valid_recipient(), 1_000, ChainType::Ethereum, 1)
        .unwrap();

    bridge
        .confirm_transfer(&id, "0xabc".to_string(), &relayer(), 2)
        .unwrap();

    // Second confirm should fail.
    let result = bridge.confirm_transfer(&id, "0xabc".to_string(), &relayer(), 3);
    assert_eq!(result, Err(BridgeError::AlreadyCompleted));
}

#[test]
fn test_refund_transfer_success() {
    let mut bridge = setup();
    let amount = 5_000u128;
    let id = bridge
        .initiate_transfer(sender(), valid_recipient(), amount, ChainType::Ethereum, 1)
        .unwrap();

    let fee = bridge.calculate_fee(amount);
    let refund = bridge.refund_transfer(&id, &relayer()).unwrap();

    assert_eq!(refund, amount - fee);
    let t = bridge.get_transfer(&id).unwrap();
    assert_eq!(t.status, TransferStatus::Refunded);
    assert_eq!(bridge.get_total_locked(), 0);
}

#[test]
fn test_refund_transfer_unauthorized_relayer() {
    let mut bridge = setup();
    let id = bridge
        .initiate_transfer(sender(), valid_recipient(), 1_000, ChainType::Ethereum, 1)
        .unwrap();

    let result = bridge.refund_transfer(&id, &stranger());
    assert_eq!(result, Err(BridgeError::UnauthorizedRelayer(stranger())));
}

#[test]
fn test_refund_after_complete_fails() {
    let mut bridge = setup();
    let id = bridge
        .initiate_transfer(sender(), valid_recipient(), 1_000, ChainType::Ethereum, 1)
        .unwrap();

    bridge
        .confirm_transfer(&id, "0xabc".to_string(), &relayer(), 2)
        .unwrap();

    let result = bridge.refund_transfer(&id, &relayer());
    assert_eq!(result, Err(BridgeError::AlreadyCompleted));
}

#[test]
fn test_get_sender_transfers() {
    let mut bridge = setup();

    bridge
        .initiate_transfer(sender(), valid_recipient(), 500, ChainType::Ethereum, 1)
        .unwrap();
    bridge
        .initiate_transfer(sender(), valid_recipient(), 600, ChainType::Base, 2)
        .unwrap();

    let transfers = bridge.get_sender_transfers(&sender());
    assert_eq!(transfers.len(), 2);
}

#[test]
fn test_get_sender_transfers_empty_for_unknown() {
    let bridge = setup();
    let transfers = bridge.get_sender_transfers(&stranger());
    assert!(transfers.is_empty());
}

#[test]
fn test_get_supported_chains_contains_added() {
    let bridge = setup();
    let chains = bridge.get_supported_chains();
    let chain_list: Vec<_> = chains.iter().map(|c| (*c).clone()).collect();
    assert!(chain_list.contains(&ChainType::Ethereum));
    assert!(chain_list.contains(&ChainType::Base));
    assert!(chain_list.contains(&ChainType::ISAChain));
}
