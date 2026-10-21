use super::*;
use crate::cross_chain::types::{BridgeError, ChainType, TransferStatus};

#[test]
fn test_new_bridge_defaults() {
    let bridge = CrossChainBridge::new(100, 100, 1_000_000, admin());
    assert_eq!(bridge.fee_rate_bps, 100);
    assert_eq!(bridge.min_transfer, 100);
    assert_eq!(bridge.max_transfer, 1_000_000);
    assert_eq!(bridge.total_locked, 0);
    assert_eq!(bridge.total_transferred, 0);
    assert!(bridge.supported_chains.contains(&ChainType::ISAChain));
}

#[test]
fn test_initiate_transfer_success() {
    let mut bridge = setup();
    let id = bridge
        .initiate_transfer(sender(), valid_recipient(), 1_000, ChainType::Ethereum, 42)
        .unwrap();

    let t = bridge.get_transfer(&id).unwrap();
    assert_eq!(t.sender, sender());
    assert_eq!(t.amount, 1_000);
    assert_eq!(t.dest_chain, ChainType::Ethereum);
    assert_eq!(t.status, TransferStatus::Locked);
    assert_eq!(bridge.get_total_locked(), 1_000);
}

#[test]
fn test_initiate_transfer_unsupported_chain() {
    let mut bridge = setup();
    let result = bridge.initiate_transfer(
        sender(),
        valid_recipient(),
        1_000,
        ChainType::Custom("Unknown".to_string()),
        1,
    );
    assert_eq!(result, Err(BridgeError::UnsupportedChain));
}

#[test]
fn test_initiate_transfer_below_minimum() {
    let mut bridge = setup();
    let result = bridge.initiate_transfer(
        sender(),
        valid_recipient(),
        50, // < min_transfer (100)
        ChainType::Ethereum,
        1,
    );
    assert_eq!(result, Err(BridgeError::InsufficientAmount));
}

#[test]
fn test_initiate_transfer_exceeds_maximum() {
    let mut bridge = setup();
    let result = bridge.initiate_transfer(
        sender(),
        valid_recipient(),
        2_000_000, // > max_transfer (1_000_000)
        ChainType::Ethereum,
        1,
    );
    assert_eq!(result, Err(BridgeError::ExceedsMaxTransfer));
}

#[test]
fn test_initiate_transfer_invalid_recipient_empty() {
    let mut bridge = setup();
    let result = bridge.initiate_transfer(
        sender(),
        String::new(),
        1_000,
        ChainType::Ethereum,
        1,
    );
    assert_eq!(result, Err(BridgeError::InvalidAddress));
}

#[test]
fn test_initiate_transfer_invalid_recipient_no_0x_prefix() {
    let mut bridge = setup();
    let result = bridge.initiate_transfer(
        sender(),
        "AbCdEf1234567890AbCdEf1234567890AbCdEf12".to_string(),
        1_000,
        ChainType::Ethereum,
        1,
    );
    assert_eq!(result, Err(BridgeError::InvalidAddress));
}

#[test]
fn test_calculate_fee_one_percent() {
    let bridge = setup(); // 100 bps = 1 %
    assert_eq!(bridge.calculate_fee(10_000), 100);
    assert_eq!(bridge.calculate_fee(1_000), 10);
    assert_eq!(bridge.calculate_fee(100), 1);
}

#[test]
fn test_calculate_fee_rounds_down() {
    let bridge = setup(); // 1 %
    // 1 % of 199 = 1.99 → floors to 1
    assert_eq!(bridge.calculate_fee(199), 1);
}

#[test]
fn test_total_locked_accumulates_across_transfers() {
    let mut bridge = setup();

    bridge
        .initiate_transfer(sender(), valid_recipient(), 1_000, ChainType::Ethereum, 1)
        .unwrap();
    bridge
        .initiate_transfer(sender(), valid_recipient(), 2_000, ChainType::Base, 2)
        .unwrap();

    assert_eq!(bridge.get_total_locked(), 3_000);
}
