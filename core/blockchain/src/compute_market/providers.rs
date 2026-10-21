use crate::compute_market::types::*;
use crate::types::*;

impl ComputeMarketState {
    /// Register a new compute provider
    #[allow(clippy::too_many_arguments)]
    pub fn register_provider(
        &mut self,
        address: Address,
        resource_types: Vec<ResourceType>,
        capacity: ComputeCapacity,
        price_per_hour: Amount,
        min_duration_secs: u64,
        max_duration_secs: u64,
        stake: Amount,
        region: Option<String>,
        endpoint: String,
    ) -> Result<(), ComputeMarketError> {
        // Check if provider already exists
        if self.providers.contains_key(&address) {
            return Err(ComputeMarketError::ProviderAlreadyRegistered);
        }

        // Validate stake
        if stake < constants::PROVIDER_MIN_STAKE {
            return Err(ComputeMarketError::InsufficientStake);
        }

        let provider = ProviderInfo {
            address,
            resource_types,
            capacity: capacity.clone(),
            available_capacity: capacity,
            price_per_hour,
            min_duration_secs,
            max_duration_secs,
            stake,
            status: ProviderStatus::Active,
            reputation: 5000, // Start at 50% (neutral)
            jobs_completed: 0,
            jobs_failed: 0,
            region,
            endpoint,
        };

        self.providers.insert(address, provider);
        self.total_provider_stake += stake;
        self.stats.total_providers += 1;
        self.stats.active_providers += 1;

        Ok(())
    }

    /// Update provider configuration
    pub fn update_provider(
        &mut self,
        address: Address,
        capacity: Option<ComputeCapacity>,
        price_per_hour: Option<Amount>,
        status: Option<ProviderStatus>,
        additional_stake: Option<Amount>,
        endpoint: Option<String>,
    ) -> Result<(), ComputeMarketError> {
        let provider = self
            .providers
            .get_mut(&address)
            .ok_or(ComputeMarketError::ProviderNotFound)?;

        if let Some(cap) = capacity {
            provider.capacity = cap.clone();
            provider.available_capacity = cap;
        }

        if let Some(price) = price_per_hour {
            provider.price_per_hour = price;
        }

        if let Some(s) = status {
            match s {
                ProviderStatus::Active | ProviderStatus::Paused => {
                    let was_active = provider.status == ProviderStatus::Active;
                    let is_active = s == ProviderStatus::Active;
                    provider.status = s;

                    // Update active provider count
                    if was_active && !is_active {
                        self.stats.active_providers = self.stats.active_providers.saturating_sub(1);
                    } else if !was_active && is_active {
                        self.stats.active_providers += 1;
                    }
                }
                _ => return Err(ComputeMarketError::InvalidStatusTransition),
            }
        }

        if let Some(stake) = additional_stake {
            provider.stake += stake;
            self.total_provider_stake += stake;
        }

        if let Some(ep) = endpoint {
            provider.endpoint = ep;
        }

        Ok(())
    }

    /// Provider initiates exit (begins unbonding period)
    pub fn provider_exit(&mut self, address: Address) -> Result<Amount, ComputeMarketError> {
        let provider = self
            .providers
            .get_mut(&address)
            .ok_or(ComputeMarketError::ProviderNotFound)?;

        // Check no active jobs
        if let Some(jobs) = self.provider_jobs.get(&address) {
            for job_id in jobs {
                if let Some(job) = self.jobs.get(job_id) {
                    if matches!(
                        job.status,
                        JobStatus::Pending | JobStatus::Matched | JobStatus::Running
                    ) {
                        return Err(ComputeMarketError::HasActiveJobs);
                    }
                }
            }
        }

        let stake = provider.stake;
        provider.status = ProviderStatus::Exited;
        self.total_provider_stake = self.total_provider_stake.saturating_sub(stake);
        self.stats.active_providers = self.stats.active_providers.saturating_sub(1);

        Ok(stake)
    }
}
