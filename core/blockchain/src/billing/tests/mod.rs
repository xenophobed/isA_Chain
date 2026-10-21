use super::*;
use crate::settlement::ServiceType;
use crate::types::Address;

pub(crate) fn provider() -> Address {
    Address::ZERO
}

pub(crate) fn user() -> Address {
    Address::new([1u8; 20])
}

pub(crate) fn model_config() -> BillingConfig {
    BillingConfig {
        service: ServiceType::ModelInference,
        provider_wallet: provider(),
        base_rate: 100,
        rate_per_token: 1,
        rate_per_second: 10,
        min_charge: 50,
        enabled: true,
    }
}

pub(crate) fn tool_config() -> BillingConfig {
    BillingConfig {
        service: ServiceType::ToolExecution,
        provider_wallet: provider(),
        base_rate: 50,
        rate_per_token: 0,
        rate_per_second: 2,
        min_charge: 20,
        enabled: true,
    }
}

pub(crate) fn compute_config() -> BillingConfig {
    BillingConfig {
        service: ServiceType::ComputeUsage,
        provider_wallet: provider(),
        base_rate: 200,
        rate_per_token: 0,
        rate_per_second: 5,
        min_charge: 100,
        enabled: true,
    }
}

pub(crate) fn storage_config() -> BillingConfig {
    BillingConfig {
        service: ServiceType::Storage,
        provider_wallet: provider(),
        base_rate: 75,
        rate_per_token: 0,
        rate_per_second: 0,
        min_charge: 10,
        enabled: true,
    }
}

mod charge_tests;
mod event_tests;
