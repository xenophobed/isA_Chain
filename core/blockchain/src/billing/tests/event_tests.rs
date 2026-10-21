use super::*;
use crate::billing::types::{BillingError, BillingMetadata};
use crate::settlement::ServiceType;
use uuid::Uuid;

#[test]
fn test_create_event() {
    let mut calc = BillingCalculator::new();
    calc.configure_service(model_config()).unwrap();

    let meta = BillingMetadata::ModelInference {
        model_id: "claude-3".to_string(),
        input_tokens: 100,
        output_tokens: 50,
        latency_ms: 800,
    };
    let event = calc.create_event(user(), meta, 1_700_000_000_000).unwrap();

    assert!(!event.event_id.is_empty());
    assert_eq!(event.service, ServiceType::ModelInference);
    assert_eq!(event.user, user());
    assert_eq!(event.provider_wallet, provider());
    assert_eq!(event.timestamp, 1_700_000_000_000);
    // base(100) + 150*1 = 250
    assert_eq!(event.amount, 250);

    // event_id must be a valid UUID
    assert!(Uuid::parse_str(&event.event_id).is_ok());
}

#[test]
fn test_service_not_configured() {
    let calc = BillingCalculator::new();
    let meta = BillingMetadata::ToolExecution {
        tool_name: "missing".to_string(),
        execution_time_ms: 100,
        success: false,
    };
    assert_eq!(
        calc.calculate_charge(&meta).unwrap_err(),
        BillingError::ServiceNotConfigured
    );
}

#[test]
fn test_billing_disabled() {
    let mut calc = BillingCalculator::new();
    calc.configure_service(BillingConfig {
        service: ServiceType::ToolExecution,
        provider_wallet: provider(),
        base_rate: 50,
        rate_per_token: 0,
        rate_per_second: 2,
        min_charge: 10,
        enabled: false,
    })
    .unwrap();

    let meta = BillingMetadata::ToolExecution {
        tool_name: "disabled_tool".to_string(),
        execution_time_ms: 500,
        success: true,
    };
    assert_eq!(
        calc.calculate_charge(&meta).unwrap_err(),
        BillingError::BillingDisabled
    );
}

#[test]
fn test_zero_usage() {
    let mut calc = BillingCalculator::new();
    calc.configure_service(model_config()).unwrap();

    let meta = BillingMetadata::ModelInference {
        model_id: "any".to_string(),
        input_tokens: 0,
        output_tokens: 0,
        latency_ms: 0,
    };
    assert_eq!(
        calc.calculate_charge(&meta).unwrap_err(),
        BillingError::ZeroUsage
    );
}
