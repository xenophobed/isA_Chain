use super::*;
use crate::settlement::ServiceType;
use crate::types::Address;

pub(crate) fn user1() -> Address {
    Address::from([0x01; 20])
}

pub(crate) fn user2() -> Address {
    Address::from([0x02; 20])
}

pub(crate) fn service_model() -> ServiceType {
    ServiceType::ModelInference
}

pub(crate) fn service_tool() -> ServiceType {
    ServiceType::ToolExecution
}

/// Build a default engine with ModelInference and ToolExecution enabled,
/// max 10 requests per period, period = 100 blocks.
pub(crate) fn setup() -> CreditBillingEngine {
    let mut engine = CreditBillingEngine::new(10, 100);
    engine.enable_service(service_model());
    engine.enable_service(service_tool());
    engine
}

mod charge_tests;
mod summary_tests;
