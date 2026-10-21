use crate::types::constants;
use super::data::TransactionData;
use super::types::{Transaction, TransactionError};

impl Transaction {
    /// Validate transaction data based on type
    pub fn validate_data(&self) -> Result<(), TransactionError> {
        match &self.data {
            TransactionData::Transfer { amount, .. } => {
                if *amount == 0 {
                    return Err(TransactionError::InvalidAmount);
                }
            }

            TransactionData::ContractDeploy { bytecode, .. } => {
                if bytecode.is_empty() {
                    return Err(TransactionError::EmptyBytecode);
                }
                if bytecode.len() > 1_000_000 {
                    // 1MB limit
                    return Err(TransactionError::BytecodeTooLarge);
                }
            }

            TransactionData::Stake { amount, validator_info } => {
                if *amount < constants::VALIDATOR_MIN_STAKE {
                    return Err(TransactionError::InsufficientStakeAmount);
                }
                if validator_info.commission_rate > 10000 {
                    // Max 100% (10000 basis points)
                    return Err(TransactionError::InvalidCommissionRate);
                }
            }

            TransactionData::Delegate { amount, .. } => {
                if *amount < constants::DELEGATION_MIN_AMOUNT {
                    return Err(TransactionError::InsufficientDelegationAmount);
                }
            }

            // ========== Compute Marketplace Validation ==========
            TransactionData::ComputeRegister {
                stake,
                resource_types,
                capacity,
                price_per_hour,
                endpoint,
                ..
            } => {
                if *stake < constants::PROVIDER_MIN_STAKE {
                    return Err(TransactionError::InsufficientProviderStake);
                }
                if resource_types.is_empty() {
                    return Err(TransactionError::NoResourceTypes);
                }
                if capacity.cpu_millicores == 0 || capacity.memory_mb == 0 {
                    return Err(TransactionError::InvalidCapacity);
                }
                if *price_per_hour == 0 {
                    return Err(TransactionError::InvalidPrice);
                }
                if endpoint.is_empty() {
                    return Err(TransactionError::InvalidEndpoint);
                }
            }

            TransactionData::ComputeRequest {
                capacity,
                max_price_per_hour,
                duration_secs,
                escrow_amount,
                ..
            } => {
                if capacity.cpu_millicores == 0 || capacity.memory_mb == 0 {
                    return Err(TransactionError::InvalidCapacity);
                }
                if *max_price_per_hour == 0 {
                    return Err(TransactionError::InvalidPrice);
                }
                if *duration_secs == 0 {
                    return Err(TransactionError::InvalidDuration);
                }
                if *escrow_amount < constants::JOB_MIN_ESCROW {
                    return Err(TransactionError::InsufficientEscrow);
                }
                // Verify escrow covers at least the minimum expected cost
                let min_cost = (*duration_secs as u128 * *max_price_per_hour) / 3600;
                if *escrow_amount < min_cost {
                    return Err(TransactionError::InsufficientEscrow);
                }
            }

            TransactionData::ComputeAccept { price_per_hour, .. } => {
                if *price_per_hour == 0 {
                    return Err(TransactionError::InvalidPrice);
                }
            }

            TransactionData::ComputeSettle { usage, .. } => {
                if usage.duration_secs == 0 {
                    return Err(TransactionError::InvalidDuration);
                }
            }

            TransactionData::ComputeResolveDispute {
                user_refund,
                provider_payment,
                slash_amount,
                ..
            } => {
                // Slash cannot exceed max slash percent of typical provider stake
                // This is a sanity check; actual validation happens in state transition
                if *slash_amount > 0 && *user_refund == 0 && *provider_payment == 0 {
                    return Err(TransactionError::InvalidDisputeResolution);
                }
            }

            _ => {} // Other transaction types are valid by default
        }

        Ok(())
    }
}
