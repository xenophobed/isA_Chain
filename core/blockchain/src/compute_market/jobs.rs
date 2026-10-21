use crate::compute_market::types::*;
use crate::types::*;

impl ComputeMarketState {
    /// Create a new compute job request
    #[allow(clippy::too_many_arguments)]
    pub fn create_job(
        &mut self,
        job_id: Hash,
        user: Address,
        provider: Option<Address>,
        resource_type: ResourceType,
        capacity: ComputeCapacity,
        max_price_per_hour: Amount,
        duration_secs: u64,
        escrow_amount: Amount,
        created_at: Timestamp,
    ) -> Result<(), ComputeMarketError> {
        // Check job doesn't already exist
        if self.jobs.contains_key(&job_id) {
            return Err(ComputeMarketError::JobAlreadyExists);
        }

        // If specific provider requested, verify they exist and are active
        if let Some(ref prov_addr) = provider {
            let prov = self
                .providers
                .get(prov_addr)
                .ok_or(ComputeMarketError::ProviderNotFound)?;

            if prov.status != ProviderStatus::Active {
                return Err(ComputeMarketError::ProviderNotActive);
            }

            // Check provider supports this resource type
            if !prov.resource_types.contains(&resource_type) {
                return Err(ComputeMarketError::ResourceTypeNotSupported);
            }

            // Check provider has capacity
            if !Self::has_capacity(&prov.available_capacity, &capacity) {
                return Err(ComputeMarketError::InsufficientCapacity);
            }
        }

        let job = ComputeJob {
            job_id,
            user,
            provider,
            resource_type,
            capacity,
            max_price_per_hour,
            duration_secs,
            escrow_amount,
            status: JobStatus::Pending,
            created_at,
            started_at: None,
            ended_at: None,
            actual_usage: None,
        };

        self.jobs.insert(job_id, job);
        self.user_jobs.entry(user).or_default().push(job_id);
        self.total_escrow += escrow_amount;
        self.stats.total_jobs_created += 1;

        Ok(())
    }

    /// Provider accepts a job
    pub fn accept_job(
        &mut self,
        job_id: Hash,
        provider: Address,
        price_per_hour: Amount,
    ) -> Result<(), ComputeMarketError> {
        let job = self
            .jobs
            .get_mut(&job_id)
            .ok_or(ComputeMarketError::JobNotFound)?;

        // Verify job is pending
        if job.status != JobStatus::Pending {
            return Err(ComputeMarketError::InvalidJobState);
        }

        // If job has a specific provider, verify it matches
        if let Some(req_provider) = job.provider {
            if req_provider != provider {
                return Err(ComputeMarketError::NotAuthorized);
            }
        }

        // Verify price is within user's max
        if price_per_hour > job.max_price_per_hour {
            return Err(ComputeMarketError::PriceExceedsMax);
        }

        // Verify provider exists and is active
        let prov = self
            .providers
            .get_mut(&provider)
            .ok_or(ComputeMarketError::ProviderNotFound)?;

        if prov.status != ProviderStatus::Active {
            return Err(ComputeMarketError::ProviderNotActive);
        }

        // Reserve capacity
        if !Self::has_capacity(&prov.available_capacity, &job.capacity) {
            return Err(ComputeMarketError::InsufficientCapacity);
        }
        Self::deduct_capacity(&mut prov.available_capacity, &job.capacity);

        // Update job
        job.provider = Some(provider);
        job.status = JobStatus::Matched;

        // Track provider's jobs
        self.provider_jobs.entry(provider).or_default().push(job_id);

        Ok(())
    }

    /// Mark job as started
    pub fn start_job(
        &mut self,
        job_id: Hash,
        provider: Address,
        started_at: Timestamp,
    ) -> Result<(), ComputeMarketError> {
        let job = self
            .jobs
            .get_mut(&job_id)
            .ok_or(ComputeMarketError::JobNotFound)?;

        // Verify job is matched
        if job.status != JobStatus::Matched {
            return Err(ComputeMarketError::InvalidJobState);
        }

        // Verify caller is the provider
        if job.provider != Some(provider) {
            return Err(ComputeMarketError::NotAuthorized);
        }

        job.status = JobStatus::Running;
        job.started_at = Some(started_at);

        Ok(())
    }

    /// Settle a completed job
    pub fn settle_job(
        &mut self,
        job_id: Hash,
        usage: ComputeUsage,
        settled_at: Timestamp,
    ) -> Result<SettlementResult, ComputeMarketError> {
        let job = self
            .jobs
            .get_mut(&job_id)
            .ok_or(ComputeMarketError::JobNotFound)?;

        // Verify job is running
        if job.status != JobStatus::Running {
            return Err(ComputeMarketError::InvalidJobState);
        }

        let provider_addr = job.provider.ok_or(ComputeMarketError::NoProvider)?;

        // Calculate payment based on actual usage
        let duration_hours = usage.duration_secs as f64 / 3600.0;
        let base_payment = (job.max_price_per_hour as f64 * duration_hours) as Amount;

        // Apply protocol fee
        let protocol_fee =
            (base_payment * constants::PROTOCOL_FEE_PERCENT as u128) / 10000;
        let provider_payment = base_payment - protocol_fee;

        // Calculate refund (escrow - payment)
        let user_refund = job.escrow_amount.saturating_sub(base_payment);

        // Update job state
        job.status = JobStatus::Completed;
        job.ended_at = Some(settled_at);
        job.actual_usage = Some(usage);

        // Update provider stats
        if let Some(provider) = self.providers.get_mut(&provider_addr) {
            provider.jobs_completed += 1;
            // Restore capacity
            Self::restore_capacity(&mut provider.available_capacity, &job.capacity);
            // Slight reputation boost
            provider.reputation = (provider.reputation + 10).min(10000);
        }

        // Update global state
        self.total_escrow = self.total_escrow.saturating_sub(job.escrow_amount);
        self.treasury_balance += protocol_fee;
        self.stats.total_jobs_completed += 1;
        self.stats.total_volume += base_payment;

        Ok(SettlementResult {
            job_id,
            provider_payment,
            user_refund,
            protocol_fee,
        })
    }

    /// Cancel a job
    pub fn cancel_job(
        &mut self,
        job_id: Hash,
        canceller: Address,
        cancelled_at: Timestamp,
    ) -> Result<Amount, ComputeMarketError> {
        let job = self
            .jobs
            .get_mut(&job_id)
            .ok_or(ComputeMarketError::JobNotFound)?;

        // Verify canceller is user or provider
        let is_user = job.user == canceller;
        let is_provider = job.provider == Some(canceller);
        if !is_user && !is_provider {
            return Err(ComputeMarketError::NotAuthorized);
        }

        // Can only cancel pending or matched jobs
        if !matches!(job.status, JobStatus::Pending | JobStatus::Matched) {
            return Err(ComputeMarketError::InvalidJobState);
        }

        // If provider cancels, restore their capacity
        if let Some(provider_addr) = job.provider {
            if let Some(provider) = self.providers.get_mut(&provider_addr) {
                Self::restore_capacity(&mut provider.available_capacity, &job.capacity);
            }
        }

        let refund = job.escrow_amount;
        job.status = JobStatus::Cancelled;
        job.ended_at = Some(cancelled_at);

        self.total_escrow = self.total_escrow.saturating_sub(refund);

        Ok(refund)
    }
}
