use crate::subnet::registry::SubnetRegistry;
use crate::types::Address;

mod provider_tests;
mod subnet_tests;

pub(crate) fn admin() -> Address {
    Address::from([0xAA; 20])
}

pub(crate) fn other_admin() -> Address {
    Address::from([0xBB; 20])
}

pub(crate) fn provider(byte: u8) -> Address {
    Address::from([byte; 20])
}

pub(crate) fn registry() -> SubnetRegistry {
    SubnetRegistry::new(admin())
}

pub(crate) fn initialized() -> SubnetRegistry {
    let mut r = registry();
    r.initialize_default_subnets(1).unwrap();
    r
}
