use super::*;
use crate::billing::types::BillingMetadata;
use crate::settlement::ServiceType;

#[test]
fn test_configure_service() {
    let mut calc = BillingCalculator::new();
    assert!(calc.configure_service(model_config()).is_ok());
    assert!(calc.get_config(&ServiceType::ModelInference).is_some());
}

#[test]
fn test_calculate_model_charge() {
    let mut calc = BillingCalculator::new();
    calc.configure_service(model_config()).unwrap();

    let meta = BillingMetadata::ModelInference {
        model_id: "gpt-4o".to_string(),
        input_tokens: 500,
        output_tokens: 300,
        latency_ms: 1200,
    };
    // base_rate(100) + (500+300)*rate_per_token(1) = 900
    let charge = calc.calculate_charge(&meta).unwrap();
    assert_eq!(charge, 900);
}

#[test]
fn test_calculate_tool_charge() {
    let mut calc = BillingCalculator::new();
    calc.configure_service(tool_config()).unwrap();

    let meta = BillingMetadata::ToolExecution {
        tool_name: "web_search".to_string(),
        execution_time_ms: 2000,
        success: true,
    };
    // base_rate(50) + 2000 * rate_per_second(2) / 1000 = 54
    let charge = calc.calculate_charge(&meta).unwrap();
    assert_eq!(charge, 54);
}

#[test]
fn test_calculate_compute_charge() {
    let mut calc = BillingCalculator::new();
    calc.configure_service(compute_config()).unwrap();

    let meta = BillingMetadata::ComputeUsage {
        resource_type: "gpu".to_string(),
        compute_seconds: 10,
        memory_mb_seconds: 512,
    };
    // base_rate(200) + 10 * rate_per_second(5) = 250
    let charge = calc.calculate_charge(&meta).unwrap();
    assert_eq!(charge, 250);
}

#[test]
fn test_calculate_storage_charge() {
    let mut calc = BillingCalculator::new();
    calc.configure_service(storage_config()).unwrap();

    let meta = BillingMetadata::Storage {
        bytes_stored: 1_000_000,
        duration_secs: 86400,
    };
    // flat base_rate(75), min_charge(10) — result is 75
    let charge = calc.calculate_charge(&meta).unwrap();
    assert_eq!(charge, 75);
}

#[test]
fn test_min_charge_applied() {
    let mut calc = BillingCalculator::new();
    // base_rate=0, rate_per_token=0, min_charge=50
    calc.configure_service(BillingConfig {
        service: ServiceType::ModelInference,
        provider_wallet: provider(),
        base_rate: 0,
        rate_per_token: 0,
        rate_per_second: 0,
        min_charge: 50,
        enabled: true,
    })
    .unwrap();

    let meta = BillingMetadata::ModelInference {
        model_id: "small-model".to_string(),
        input_tokens: 1,
        output_tokens: 1,
        latency_ms: 10,
    };
    // raw = 0 + 2*0 = 0, clamped to min_charge(50)
    let charge = calc.calculate_charge(&meta).unwrap();
    assert_eq!(charge, 50);
}

#[test]
fn test_multiple_services() {
    let mut calc = BillingCalculator::new();
    calc.configure_service(model_config()).unwrap();
    calc.configure_service(tool_config()).unwrap();
    calc.configure_service(compute_config()).unwrap();

    assert!(calc.is_enabled(&ServiceType::ModelInference));
    assert!(calc.is_enabled(&ServiceType::ToolExecution));
    assert!(calc.is_enabled(&ServiceType::ComputeUsage));
    assert!(!calc.is_enabled(&ServiceType::Storage));

    // Each service charges independently
    let model_charge = calc
        .calculate_charge(&BillingMetadata::ModelInference {
            model_id: "m".to_string(),
            input_tokens: 10,
            output_tokens: 10,
            latency_ms: 100,
        })
        .unwrap();
    assert_eq!(model_charge, 120); // 100 + 20*1

    let tool_charge = calc
        .calculate_charge(&BillingMetadata::ToolExecution {
            tool_name: "t".to_string(),
            execution_time_ms: 1000,
            success: true,
        })
        .unwrap();
    assert_eq!(tool_charge, 52); // 50 + 1000*2/1000

    let compute_charge = calc
        .calculate_charge(&BillingMetadata::ComputeUsage {
            resource_type: "cpu".to_string(),
            compute_seconds: 3,
            memory_mb_seconds: 256,
        })
        .unwrap();
    assert_eq!(compute_charge, 215); // 200 + 3*5
}

#[test]
fn test_generic_metadata() {
    let mut calc = BillingCalculator::new();
    calc.configure_service(BillingConfig {
        service: ServiceType::Custom("generic".to_string()),
        provider_wallet: provider(),
        base_rate: 42,
        rate_per_token: 0,
        rate_per_second: 0,
        min_charge: 1,
        enabled: true,
    })
    .unwrap();

    let meta = BillingMetadata::Generic {
        description: "ad-hoc charge".to_string(),
    };
    let charge = calc.calculate_charge(&meta).unwrap();
    assert_eq!(charge, 42);
}
