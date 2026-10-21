use crate::agent_registry::*;
use crate::types::{Address, Hash};

pub fn make_address(seed: u8) -> Address {
    Address::new([seed; 20])
}

pub fn make_hash(seed: u8) -> Hash {
    Hash::new([seed; 32])
}

pub fn default_pricing() -> AgentPricing {
    AgentPricing {
        base_fee: 100,
        per_token_fee: 1,
        per_second_fee: 10,
        minimum_charge: 50,
    }
}

pub fn register_basic(
    registry: &mut AgentRegistry,
    agent_id: Hash,
    owner: Address,
) -> Result<(), RegistryError> {
    registry.register(
        agent_id,
        owner,
        make_address(0xAA),
        "TestAgent".to_string(),
        "A test agent".to_string(),
        vec![AgentCapability::TextGeneration],
        default_pricing(),
        1,
    )
}

mod query_and_update_tests;
mod registration_tests;
