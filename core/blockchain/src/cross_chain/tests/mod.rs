use crate::cross_chain::bridge::CrossChainBridge;
use crate::cross_chain::types::ChainType;
use crate::types::Address;

mod lifecycle_tests;
mod transfer_tests;

// ---- helpers -----------------------------------------------------------

pub(crate) fn admin() -> Address {
    Address::from([0xAA; 20])
}

pub(crate) fn relayer() -> Address {
    Address::from([0xBB; 20])
}

pub(crate) fn sender() -> Address {
    Address::from([0xCC; 20])
}

pub(crate) fn stranger() -> Address {
    Address::from([0xDD; 20])
}

/// Default bridge: 1 % fee, 100–1 000 000 ISA range, Ethereum + Base supported.
pub(crate) fn setup() -> CrossChainBridge {
    let mut bridge = CrossChainBridge::new(100, 100, 1_000_000, admin());
    bridge.add_supported_chain(ChainType::Ethereum);
    bridge.add_supported_chain(ChainType::Base);
    bridge.add_relayer(relayer(), &admin()).unwrap();
    bridge
}

pub(crate) fn valid_recipient() -> String {
    "0xAbCdEf1234567890AbCdEf1234567890AbCdEf12".to_string()
}
