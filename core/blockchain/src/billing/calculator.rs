use crate::settlement::ServiceType;
use crate::types::{Address, Amount, Timestamp};
use std::collections::HashMap;
use uuid::Uuid;

use super::types::{BillingConfig, BillingError, BillingHookEvent, BillingMetadata};

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
