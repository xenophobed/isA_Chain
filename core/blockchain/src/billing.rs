use crate::settlement::ServiceType;
use crate::types::{Address, Amount, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// BillingMetadata
// ============================================================================

/// Service-specific metadata attached to a billing hook event.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BillingMetadata {
    /// isA_Model — model inference billing metadata
    ModelInference {
        model_id: String,
        input_tokens: u64,
        output_tokens: u64,
        latency_ms: u64,
    },
    /// isA_MCP — tool execution billing metadata
    ToolExecution {
        tool_name: String,
        execution_time_ms: u64,
        success: bool,
    },
    /// isA_OS — compute usage billing metadata
    ComputeUsage {
        resource_type: String,
        compute_seconds: u64,
        memory_mb_seconds: u64,
    },
    /// isA_Data — storage billing metadata
    Storage {
        bytes_stored: u64,
        duration_secs: u64,
    },
    /// isA_Agent — agent runtime billing metadata
    AgentRuntime {
        agent_id: String,
        runtime_secs: u64,
        actions_count: u64,
    },
    /// Catch-all for extensibility
    Generic { description: String },
}

impl BillingMetadata {
    /// Returns the `ServiceType` that corresponds to this metadata variant.
    pub fn service_type(&self) -> ServiceType {
        match self {
            BillingMetadata::ModelInference { .. } => ServiceType::ModelInference,
            BillingMetadata::ToolExecution { .. } => ServiceType::ToolExecution,
            BillingMetadata::ComputeUsage { .. } => ServiceType::ComputeUsage,
            BillingMetadata::Storage { .. } => ServiceType::Storage,
            BillingMetadata::AgentRuntime { .. } => ServiceType::AgentRuntime,
            BillingMetadata::Generic { .. } => ServiceType::Custom("generic".to_string()),
        }
    }
}

// ============================================================================
// BillingHookEvent
// ============================================================================

/// An event emitted by a service (isA_Model, isA_MCP, isA_OS, …) that drives
/// an on-chain settlement via the `SettlementBridge`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillingHookEvent {
    /// Unique event ID in UUID format.
    pub event_id: String,
    /// Which isA service generated this event.
    pub service: ServiceType,
    /// Address of the user being charged.
    pub user: Address,
    /// Provider's on-chain wallet address (receives the net payment).
    pub provider_wallet: Address,
    /// Billed amount in credits.
    pub amount: Amount,
    /// Unix timestamp (milliseconds) when the event was generated.
    pub timestamp: Timestamp,
    /// Service-specific billing metadata.
    pub metadata: BillingMetadata,
}

// ============================================================================
// BillingConfig
// ============================================================================

/// Per-service billing configuration used by `BillingCalculator`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillingConfig {
    /// Which isA service this config applies to.
    pub service: ServiceType,
    /// Provider's on-chain wallet address.
    pub provider_wallet: Address,
    /// Base rate charged per billing event (in credits).
    pub base_rate: Amount,
    /// Additional rate per token (for model inference).
    pub rate_per_token: Amount,
    /// Additional rate per second of compute/execution time.
    pub rate_per_second: Amount,
    /// Minimum charge per event (floor applied after formula).
    pub min_charge: Amount,
    /// Whether billing is active for this service.
    pub enabled: bool,
}

// ============================================================================
// BillingError
// ============================================================================

/// Errors returned by `BillingCalculator`.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BillingError {
    #[error("service not configured")]
    ServiceNotConfigured,

    #[error("metadata does not match configured service")]
    InvalidMetadata,

    #[error("usage metrics are zero — nothing to bill")]
    ZeroUsage,

    #[error("billing is disabled for this service")]
    BillingDisabled,
}

// ============================================================================
// BillingCalculator
// ============================================================================

/// Calculates charges and constructs `BillingHookEvent`s for each service.
pub struct BillingCalculator {
    configs: HashMap<ServiceType, BillingConfig>,
}

impl BillingCalculator {
    /// Create a new, empty `BillingCalculator`.
    pub fn new() -> Self {
        BillingCalculator {
            configs: HashMap::new(),
        }
    }

    /// Register or update the billing configuration for a service.
    pub fn configure_service(&mut self, config: BillingConfig) -> Result<(), BillingError> {
        self.configs.insert(config.service.clone(), config);
        Ok(())
    }

    /// Return the configuration for `service`, if any.
    pub fn get_config(&self, service: &ServiceType) -> Option<&BillingConfig> {
        self.configs.get(service)
    }

    /// Return `true` if billing is configured **and** enabled for `service`.
    pub fn is_enabled(&self, service: &ServiceType) -> bool {
        self.configs
            .get(service)
            .map(|c| c.enabled)
            .unwrap_or(false)
    }

    /// Calculate the charge (in credits) for the given `metadata`.
    ///
    /// Charge formulas:
    /// - `ModelInference`:  `base_rate + (input_tokens + output_tokens) * rate_per_token`
    /// - `ToolExecution`:   `base_rate + execution_time_ms * rate_per_second / 1000`
    /// - `ComputeUsage`:    `base_rate + compute_seconds * rate_per_second`
    /// - `Storage`:         `base_rate` (flat; no per-unit formula specified)
    /// - `AgentRuntime`:    `base_rate + runtime_secs * rate_per_second`
    /// - `Generic`:         `base_rate`
    ///
    /// The result is clamped to at least `min_charge`.
    pub fn calculate_charge(&self, metadata: &BillingMetadata) -> Result<Amount, BillingError> {
        let service = metadata.service_type();
        let config = self.configs.get(&service).ok_or(BillingError::ServiceNotConfigured)?;

        if !config.enabled {
            return Err(BillingError::BillingDisabled);
        }

        let raw: Amount = match metadata {
            BillingMetadata::ModelInference {
                input_tokens,
                output_tokens,
                ..
            } => {
                let total_tokens = input_tokens + output_tokens;
                if total_tokens == 0 {
                    return Err(BillingError::ZeroUsage);
                }
                config.base_rate + (total_tokens as u128) * config.rate_per_token
            }

            BillingMetadata::ToolExecution {
                execution_time_ms, ..
            } => {
                if *execution_time_ms == 0 {
                    return Err(BillingError::ZeroUsage);
                }
                config.base_rate + (*execution_time_ms as u128) * config.rate_per_second / 1_000
            }

            BillingMetadata::ComputeUsage { compute_seconds, .. } => {
                if *compute_seconds == 0 {
                    return Err(BillingError::ZeroUsage);
                }
                config.base_rate + (*compute_seconds as u128) * config.rate_per_second
            }

            BillingMetadata::Storage { .. } => config.base_rate,

            BillingMetadata::AgentRuntime { runtime_secs, .. } => {
                config.base_rate + (*runtime_secs as u128) * config.rate_per_second
            }

            BillingMetadata::Generic { .. } => config.base_rate,
        };

        Ok(raw.max(config.min_charge))
    }

    /// Build a fully-populated `BillingHookEvent` for the given user and metadata.
    pub fn create_event(
        &self,
        user: Address,
        metadata: BillingMetadata,
        timestamp: Timestamp,
    ) -> Result<BillingHookEvent, BillingError> {
        let service = metadata.service_type();
        let config = self.configs.get(&service).ok_or(BillingError::ServiceNotConfigured)?;

        if !config.enabled {
            return Err(BillingError::BillingDisabled);
        }

        let amount = self.calculate_charge(&metadata)?;

        Ok(BillingHookEvent {
            event_id: Uuid::new_v4().to_string(),
            service,
            user,
            provider_wallet: config.provider_wallet,
            amount,
            timestamp,
            metadata,
        })
    }
}

impl Default for BillingCalculator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Address;

    fn provider() -> Address {
        Address::ZERO
    }

    fn user() -> Address {
        Address::new([1u8; 20])
    }

    fn model_config() -> BillingConfig {
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

    fn tool_config() -> BillingConfig {
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

    fn compute_config() -> BillingConfig {
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

    fn storage_config() -> BillingConfig {
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

    // -------------------------------------------------------------------------
    // test_configure_service
    // -------------------------------------------------------------------------
    #[test]
    fn test_configure_service() {
        let mut calc = BillingCalculator::new();
        assert!(calc.configure_service(model_config()).is_ok());
        assert!(calc.get_config(&ServiceType::ModelInference).is_some());
    }

    // -------------------------------------------------------------------------
    // test_calculate_model_charge
    // -------------------------------------------------------------------------
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

    // -------------------------------------------------------------------------
    // test_calculate_tool_charge
    // -------------------------------------------------------------------------
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

    // -------------------------------------------------------------------------
    // test_calculate_compute_charge
    // -------------------------------------------------------------------------
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

    // -------------------------------------------------------------------------
    // test_calculate_storage_charge
    // -------------------------------------------------------------------------
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

    // -------------------------------------------------------------------------
    // test_min_charge_applied
    // -------------------------------------------------------------------------
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

    // -------------------------------------------------------------------------
    // test_create_event
    // -------------------------------------------------------------------------
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

    // -------------------------------------------------------------------------
    // test_service_not_configured
    // -------------------------------------------------------------------------
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

    // -------------------------------------------------------------------------
    // test_billing_disabled
    // -------------------------------------------------------------------------
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

    // -------------------------------------------------------------------------
    // test_zero_usage
    // -------------------------------------------------------------------------
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

    // -------------------------------------------------------------------------
    // test_multiple_services
    // -------------------------------------------------------------------------
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

    // -------------------------------------------------------------------------
    // test_generic_metadata
    // -------------------------------------------------------------------------
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
}
