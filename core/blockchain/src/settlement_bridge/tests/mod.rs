use super::*;
use crate::settlement::ServiceType;
use crate::types::{Address, Amount};
use std::collections::HashMap;

pub(crate) fn user() -> Address {
    Address::from([0x11; 20])
}

pub(crate) fn provider() -> Address {
    Address::from([0x22; 20])
}

pub(crate) fn make_event(id: &str, amount: Amount) -> BillingEvent {
    BillingEvent {
        event_id: id.to_string(),
        user: user(),
        provider: provider(),
        amount,
        service_type: ServiceType::ModelInference,
        timestamp: 1_000,
        provider_wallet: provider(),
        metadata: HashMap::new(),
    }
}

pub(crate) fn bridge() -> SettlementBridge {
    SettlementBridge::default()
}

mod batch_tests;
mod ingest_tests;
