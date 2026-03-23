//! Compute Marketplace State Management
//!
//! This module handles the on-chain state for the decentralized compute marketplace.
//! It manages provider registration, job lifecycle, escrow, and dispute resolution.

use crate::types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Compute marketplace state
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComputeMarketState {
    /// Registered providers by address
    pub providers: HashMap<Address, ProviderInfo>,

    /// Active jobs by job ID
    pub jobs: HashMap<Hash, ComputeJob>,

    /// Jobs by user address (for lookup)
    pub user_jobs: HashMap<Address, Vec<Hash>>,

    /// Jobs by provider address (for lookup)
    pub provider_jobs: HashMap<Address, Vec<Hash>>,

    /// Active disputes by dispute ID
    pub disputes: HashMap<Hash, ComputeDispute>,

    /// Protocol treasury balance (from fees)
    pub treasury_balance: Amount,

    /// Total value locked in escrow
    pub total_escrow: Amount,

    /// Total provider stakes
    pub total_provider_stake: Amount,

    /// Marketplace statistics
    pub stats: MarketStats,
}

/// Marketplace statistics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MarketStats {
    pub total_providers: u64,
    pub active_providers: u64,
    pub total_jobs_created: u64,
    pub total_jobs_completed: u64,
    pub total_jobs_failed: u64,
    pub total_jobs_disputed: u64,
    pub total_volume: Amount,
}

impl ComputeMarketState {
    pub fn new() -> Self {
        Self::default()
    }

    // ========================================================================
    // Provider Operations
    // ========================================================================

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

        // Note: In production, this would initiate an unbonding period
        // For now, we return the stake amount to be released
        Ok(stake)
    }

    // ========================================================================
    // Job Operations
    // ========================================================================

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

    // ========================================================================
    // Dispute Operations
    // ========================================================================

    /// Open a dispute on a job
    pub fn open_dispute(
        &mut self,
        dispute_id: Hash,
        job_id: Hash,
        initiator: Address,
        dispute_type: DisputeType,
        evidence_hash: Hash,
        created_at: Timestamp,
    ) -> Result<(), ComputeMarketError> {
        let job = self
            .jobs
            .get_mut(&job_id)
            .ok_or(ComputeMarketError::JobNotFound)?;

        // Verify initiator is user or provider
        let is_user = job.user == initiator;
        let is_provider = job.provider == Some(initiator);
        if !is_user && !is_provider {
            return Err(ComputeMarketError::NotAuthorized);
        }

        // Can only dispute running or completed jobs
        if !matches!(job.status, JobStatus::Running | JobStatus::Completed) {
            return Err(ComputeMarketError::InvalidJobState);
        }

        // Check dispute window for completed jobs
        if job.status == JobStatus::Completed {
            if let Some(ended_at) = job.ended_at {
                if created_at > ended_at + constants::DISPUTE_WINDOW_SECS * 1000 {
                    return Err(ComputeMarketError::DisputeWindowExpired);
                }
            }
        }

        let dispute = ComputeDispute {
            dispute_id,
            job_id,
            initiator,
            dispute_type,
            evidence_hash,
            created_at,
            deadline: created_at + constants::DISPUTE_WINDOW_SECS * 1000,
            resolution: None,
        };

        job.status = JobStatus::Disputed;
        self.disputes.insert(dispute_id, dispute);
        self.stats.total_jobs_disputed += 1;

        Ok(())
    }

    /// Resolve a dispute
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_dispute(
        &mut self,
        dispute_id: Hash,
        winner: Address,
        user_refund: Amount,
        provider_payment: Amount,
        slash_amount: Amount,
        resolved_by: Address,
        resolved_at: Timestamp,
    ) -> Result<DisputeResolution, ComputeMarketError> {
        let dispute = self
            .disputes
            .get_mut(&dispute_id)
            .ok_or(ComputeMarketError::DisputeNotFound)?;

        if dispute.resolution.is_some() {
            return Err(ComputeMarketError::DisputeAlreadyResolved);
        }

        let job = self
            .jobs
            .get_mut(&dispute.job_id)
            .ok_or(ComputeMarketError::JobNotFound)?;

        // Apply slash to provider if any
        if slash_amount > 0 {
            if let Some(provider_addr) = job.provider {
                if let Some(provider) = self.providers.get_mut(&provider_addr) {
                    let actual_slash = slash_amount.min(provider.stake);
                    provider.stake = provider.stake.saturating_sub(actual_slash);
                    self.total_provider_stake =
                        self.total_provider_stake.saturating_sub(actual_slash);
                    self.treasury_balance += actual_slash; // Slashed funds go to treasury

                    // Reputation hit
                    provider.reputation = provider.reputation.saturating_sub(500);
                    provider.jobs_failed += 1;
                }
            }
        }

        let resolution = DisputeResolution {
            winner,
            user_refund,
            provider_payment,
            slash_amount,
            resolved_by,
            resolved_at,
        };

        dispute.resolution = Some(resolution.clone());

        // Update job status based on outcome
        if user_refund > provider_payment {
            job.status = JobStatus::Failed;
            self.stats.total_jobs_failed += 1;
        } else {
            job.status = JobStatus::Completed;
            self.stats.total_jobs_completed += 1;
        }

        // Release escrow
        self.total_escrow = self.total_escrow.saturating_sub(job.escrow_amount);

        Ok(resolution)
    }

    // ========================================================================
    // Helper Functions
    // ========================================================================

    fn has_capacity(available: &ComputeCapacity, required: &ComputeCapacity) -> bool {
        available.cpu_millicores >= required.cpu_millicores
            && available.memory_mb >= required.memory_mb
            && available.storage_gb >= required.storage_gb
            && available.gpu_memory_mb >= required.gpu_memory_mb
    }

    fn deduct_capacity(available: &mut ComputeCapacity, used: &ComputeCapacity) {
        available.cpu_millicores = available.cpu_millicores.saturating_sub(used.cpu_millicores);
        available.memory_mb = available.memory_mb.saturating_sub(used.memory_mb);
        available.storage_gb = available.storage_gb.saturating_sub(used.storage_gb);
        available.gpu_memory_mb = available.gpu_memory_mb.saturating_sub(used.gpu_memory_mb);
    }

    fn restore_capacity(available: &mut ComputeCapacity, released: &ComputeCapacity) {
        available.cpu_millicores += released.cpu_millicores;
        available.memory_mb += released.memory_mb;
        available.storage_gb += released.storage_gb;
        available.gpu_memory_mb += released.gpu_memory_mb;
    }

    // ========================================================================
    // Query Functions
    // ========================================================================

    /// Get provider by address
    pub fn get_provider(&self, address: &Address) -> Option<&ProviderInfo> {
        self.providers.get(address)
    }

    /// List active providers
    pub fn list_active_providers(&self) -> Vec<&ProviderInfo> {
        self.providers
            .values()
            .filter(|p| p.status == ProviderStatus::Active)
            .collect()
    }

    /// List providers by resource type
    pub fn list_providers_by_resource(&self, resource_type: &ResourceType) -> Vec<&ProviderInfo> {
        self.providers
            .values()
            .filter(|p| {
                p.status == ProviderStatus::Active && p.resource_types.contains(resource_type)
            })
            .collect()
    }

    /// Get job by ID
    pub fn get_job(&self, job_id: &Hash) -> Option<&ComputeJob> {
        self.jobs.get(job_id)
    }

    /// List jobs for user
    pub fn list_user_jobs(&self, user: &Address) -> Vec<&ComputeJob> {
        self.user_jobs
            .get(user)
            .map(|ids| ids.iter().filter_map(|id| self.jobs.get(id)).collect())
            .unwrap_or_default()
    }

    /// List jobs for provider
    pub fn list_provider_jobs(&self, provider: &Address) -> Vec<&ComputeJob> {
        self.provider_jobs
            .get(provider)
            .map(|ids| ids.iter().filter_map(|id| self.jobs.get(id)).collect())
            .unwrap_or_default()
    }

    /// Find matching providers for a job request
    pub fn find_matching_providers(
        &self,
        resource_type: &ResourceType,
        capacity: &ComputeCapacity,
        max_price: Amount,
    ) -> Vec<&ProviderInfo> {
        self.providers
            .values()
            .filter(|p| {
                p.status == ProviderStatus::Active
                    && p.resource_types.contains(resource_type)
                    && p.price_per_hour <= max_price
                    && Self::has_capacity(&p.available_capacity, capacity)
            })
            .collect()
    }
}

/// Settlement result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettlementResult {
    pub job_id: Hash,
    pub provider_payment: Amount,
    pub user_refund: Amount,
    pub protocol_fee: Amount,
}

/// Compute market errors
#[derive(Debug, thiserror::Error)]
pub enum ComputeMarketError {
    #[error("Provider already registered")]
    ProviderAlreadyRegistered,

    #[error("Provider not found")]
    ProviderNotFound,

    #[error("Provider not active")]
    ProviderNotActive,

    #[error("Insufficient stake")]
    InsufficientStake,

    #[error("Invalid status transition")]
    InvalidStatusTransition,

    #[error("Provider has active jobs")]
    HasActiveJobs,

    #[error("Job already exists")]
    JobAlreadyExists,

    #[error("Job not found")]
    JobNotFound,

    #[error("Invalid job state for this operation")]
    InvalidJobState,

    #[error("Resource type not supported by provider")]
    ResourceTypeNotSupported,

    #[error("Insufficient provider capacity")]
    InsufficientCapacity,

    #[error("Not authorized")]
    NotAuthorized,

    #[error("Price exceeds user's maximum")]
    PriceExceedsMax,

    #[error("No provider assigned")]
    NoProvider,

    #[error("Dispute window expired")]
    DisputeWindowExpired,

    #[error("Dispute not found")]
    DisputeNotFound,

    #[error("Dispute already resolved")]
    DisputeAlreadyResolved,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_capacity() -> ComputeCapacity {
        ComputeCapacity::standard_vm()
    }

    #[test]
    fn test_provider_registration() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);

        let result = market.register_provider(
            provider_addr,
            vec![ResourceType::VM, ResourceType::Browser],
            create_test_capacity(),
            1_000_000_000_000_000_000, // 1 ISA per hour
            3600,                       // 1 hour min
            86400,                      // 24 hours max
            constants::PROVIDER_MIN_STAKE,
            Some("us-west".to_string()),
            "https://provider.example.com".to_string(),
        );

        assert!(result.is_ok());
        assert_eq!(market.stats.total_providers, 1);
        assert_eq!(market.stats.active_providers, 1);

        // Should fail if registering same provider again
        let result2 = market.register_provider(
            provider_addr,
            vec![ResourceType::VM],
            create_test_capacity(),
            1_000_000_000_000_000_000,
            3600,
            86400,
            constants::PROVIDER_MIN_STAKE,
            None,
            "https://provider2.example.com".to_string(),
        );
        assert!(result2.is_err());
    }

    #[test]
    fn test_job_lifecycle() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job-1");

        // Register provider
        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        // Create job
        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000, // 2 ISA escrow
                1000000,
            )
            .unwrap();

        assert_eq!(market.stats.total_jobs_created, 1);

        // Accept job
        market
            .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
            .unwrap();

        let job = market.get_job(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Matched);

        // Start job
        market.start_job(job_id, provider_addr, 1000001).unwrap();

        let job = market.get_job(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Running);

        // Settle job
        let result = market
            .settle_job(
                job_id,
                ComputeUsage {
                    duration_secs: 3600,
                    cpu_seconds: 3600,
                    memory_mb_seconds: 2048 * 3600,
                    network_bytes: 1_000_000,
                    storage_bytes: 10_000_000_000,
                },
                2000000,
            )
            .unwrap();

        assert!(result.provider_payment > 0);
        assert_eq!(market.stats.total_jobs_completed, 1);
    }

    // ========================================================================
    // Provider Registration Tests
    // ========================================================================

    #[test]
    fn test_provider_registration_insufficient_stake() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);

        let result = market.register_provider(
            provider_addr,
            vec![ResourceType::VM],
            create_test_capacity(),
            1_000_000_000_000_000_000,
            3600,
            86400,
            constants::PROVIDER_MIN_STAKE - 1, // Insufficient stake
            None,
            "https://provider.example.com".to_string(),
        );

        assert!(matches!(result, Err(ComputeMarketError::InsufficientStake)));
    }

    #[test]
    fn test_provider_update() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);

        // Register provider
        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        // Update price
        let new_price = 2_000_000_000_000_000_000u128;
        market
            .update_provider(
                provider_addr,
                None,
                Some(new_price),
                None,
                None,
                None,
            )
            .unwrap();

        let provider = market.get_provider(&provider_addr).unwrap();
        assert_eq!(provider.price_per_hour, new_price);

        // Update status to paused
        market
            .update_provider(
                provider_addr,
                None,
                None,
                Some(ProviderStatus::Paused),
                None,
                None,
            )
            .unwrap();

        let provider = market.get_provider(&provider_addr).unwrap();
        assert_eq!(provider.status, ProviderStatus::Paused);
        assert_eq!(market.stats.active_providers, 0);

        // Update status back to active
        market
            .update_provider(
                provider_addr,
                None,
                None,
                Some(ProviderStatus::Active),
                None,
                None,
            )
            .unwrap();

        assert_eq!(market.stats.active_providers, 1);
    }

    #[test]
    fn test_provider_update_nonexistent() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);

        let result = market.update_provider(
            provider_addr,
            None,
            Some(1_000_000_000_000_000_000),
            None,
            None,
            None,
        );

        assert!(matches!(result, Err(ComputeMarketError::ProviderNotFound)));
    }

    #[test]
    fn test_provider_exit_success() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        let stake_returned = market.provider_exit(provider_addr).unwrap();
        assert_eq!(stake_returned, constants::PROVIDER_MIN_STAKE);

        let provider = market.get_provider(&provider_addr).unwrap();
        assert_eq!(provider.status, ProviderStatus::Exited);
        assert_eq!(market.stats.active_providers, 0);
    }

    #[test]
    fn test_provider_exit_with_active_job() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        // Register provider
        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        // Create and accept job
        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        market
            .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
            .unwrap();

        // Try to exit - should fail
        let result = market.provider_exit(provider_addr);
        assert!(matches!(result, Err(ComputeMarketError::HasActiveJobs)));
    }

    // ========================================================================
    // Job Creation Tests
    // ========================================================================

    #[test]
    fn test_create_job_nonexistent_provider() {
        let mut market = ComputeMarketState::new();
        let user_addr = Address::from([2u8; 20]);
        let nonexistent_provider = Address::from([99u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        let result = market.create_job(
            job_id,
            user_addr,
            Some(nonexistent_provider),
            ResourceType::VM,
            ComputeCapacity::new(1000, 2048, 10),
            1_000_000_000_000_000_000,
            3600,
            2_000_000_000_000_000_000,
            1000000,
        );

        assert!(matches!(result, Err(ComputeMarketError::ProviderNotFound)));
    }

    #[test]
    fn test_create_job_inactive_provider() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        // Register and pause provider
        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        market
            .update_provider(
                provider_addr,
                None,
                None,
                Some(ProviderStatus::Paused),
                None,
                None,
            )
            .unwrap();

        // Try to create job
        let result = market.create_job(
            job_id,
            user_addr,
            Some(provider_addr),
            ResourceType::VM,
            ComputeCapacity::new(1000, 2048, 10),
            1_000_000_000_000_000_000,
            3600,
            2_000_000_000_000_000_000,
            1000000,
        );

        assert!(matches!(result, Err(ComputeMarketError::ProviderNotActive)));
    }

    #[test]
    fn test_create_job_unsupported_resource_type() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        // Register provider with only VM support
        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        // Try to create Browser job
        let result = market.create_job(
            job_id,
            user_addr,
            Some(provider_addr),
            ResourceType::Browser, // Not supported
            ComputeCapacity::new(1000, 2048, 10),
            1_000_000_000_000_000_000,
            3600,
            2_000_000_000_000_000_000,
            1000000,
        );

        assert!(matches!(
            result,
            Err(ComputeMarketError::ResourceTypeNotSupported)
        ));
    }

    #[test]
    fn test_create_job_duplicate_id() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        // Create first job
        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        // Try to create duplicate
        let result = market.create_job(
            job_id,
            user_addr,
            Some(provider_addr),
            ResourceType::VM,
            ComputeCapacity::new(1000, 2048, 10),
            1_000_000_000_000_000_000,
            3600,
            2_000_000_000_000_000_000,
            1000000,
        );

        assert!(matches!(result, Err(ComputeMarketError::JobAlreadyExists)));
    }

    #[test]
    fn test_create_job_without_specific_provider() {
        let mut market = ComputeMarketState::new();
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        // Create job without specifying provider (marketplace matching)
        let result = market.create_job(
            job_id,
            user_addr,
            None, // No specific provider
            ResourceType::VM,
            ComputeCapacity::new(1000, 2048, 10),
            1_000_000_000_000_000_000,
            3600,
            2_000_000_000_000_000_000,
            1000000,
        );

        assert!(result.is_ok());
        let job = market.get_job(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Pending);
        assert!(job.provider.is_none());
    }

    // ========================================================================
    // Job Accept Tests
    // ========================================================================

    #[test]
    fn test_accept_job_wrong_provider() {
        let mut market = ComputeMarketState::new();
        let provider1 = Address::from([1u8; 20]);
        let provider2 = Address::from([2u8; 20]);
        let user_addr = Address::from([3u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        // Register both providers
        for (addr, idx) in [(provider1, 1), (provider2, 2)] {
            market
                .register_provider(
                    addr,
                    vec![ResourceType::VM],
                    create_test_capacity(),
                    1_000_000_000_000_000_000,
                    3600,
                    86400,
                    constants::PROVIDER_MIN_STAKE,
                    None,
                    format!("https://provider{}.example.com", idx),
                )
                .unwrap();
        }

        // Create job for provider1
        market
            .create_job(
                job_id,
                user_addr,
                Some(provider1),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        // Provider2 tries to accept
        let result = market.accept_job(job_id, provider2, 1_000_000_000_000_000_000);
        assert!(matches!(result, Err(ComputeMarketError::NotAuthorized)));
    }

    #[test]
    fn test_accept_job_price_too_high() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000, // Max 1 ISA/hour
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        // Try to accept at higher price
        let result = market.accept_job(job_id, provider_addr, 2_000_000_000_000_000_000);
        assert!(matches!(result, Err(ComputeMarketError::PriceExceedsMax)));
    }

    #[test]
    fn test_accept_job_already_accepted() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        // Accept once
        market
            .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
            .unwrap();

        // Try to accept again
        let result = market.accept_job(job_id, provider_addr, 1_000_000_000_000_000_000);
        assert!(matches!(result, Err(ComputeMarketError::InvalidJobState)));
    }

    // ========================================================================
    // Job Start Tests
    // ========================================================================

    #[test]
    fn test_start_job_wrong_provider() {
        let mut market = ComputeMarketState::new();
        let provider1 = Address::from([1u8; 20]);
        let provider2 = Address::from([2u8; 20]);
        let user_addr = Address::from([3u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        for (addr, idx) in [(provider1, 1), (provider2, 2)] {
            market
                .register_provider(
                    addr,
                    vec![ResourceType::VM],
                    create_test_capacity(),
                    1_000_000_000_000_000_000,
                    3600,
                    86400,
                    constants::PROVIDER_MIN_STAKE,
                    None,
                    format!("https://provider{}.example.com", idx),
                )
                .unwrap();
        }

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider1),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        market
            .accept_job(job_id, provider1, 1_000_000_000_000_000_000)
            .unwrap();

        // Provider2 tries to start
        let result = market.start_job(job_id, provider2, 1000001);
        assert!(matches!(result, Err(ComputeMarketError::NotAuthorized)));
    }

    #[test]
    fn test_start_job_not_matched() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        // Try to start without accepting
        let result = market.start_job(job_id, provider_addr, 1000001);
        assert!(matches!(result, Err(ComputeMarketError::InvalidJobState)));
    }

    // ========================================================================
    // Job Settlement Tests
    // ========================================================================

    #[test]
    fn test_settle_job_not_running() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        // Try to settle without starting
        let result = market.settle_job(
            job_id,
            ComputeUsage {
                duration_secs: 3600,
                cpu_seconds: 3600,
                memory_mb_seconds: 2048 * 3600,
                network_bytes: 1_000_000,
                storage_bytes: 10_000_000_000,
            },
            2000000,
        );

        assert!(matches!(result, Err(ComputeMarketError::InvalidJobState)));
    }

    #[test]
    fn test_settle_job_protocol_fee() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        market
            .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
            .unwrap();
        market.start_job(job_id, provider_addr, 1000001).unwrap();

        let initial_treasury = market.treasury_balance;

        let result = market
            .settle_job(
                job_id,
                ComputeUsage {
                    duration_secs: 3600,
                    cpu_seconds: 3600,
                    memory_mb_seconds: 2048 * 3600,
                    network_bytes: 1_000_000,
                    storage_bytes: 10_000_000_000,
                },
                2000000,
            )
            .unwrap();

        // Verify protocol fee was collected
        assert!(result.protocol_fee > 0);
        assert_eq!(
            market.treasury_balance,
            initial_treasury + result.protocol_fee
        );

        // Verify fee is 2.5%
        let total_payment = result.provider_payment + result.protocol_fee;
        let expected_fee = (total_payment * constants::PROTOCOL_FEE_PERCENT as u128) / 10000;
        assert_eq!(result.protocol_fee, expected_fee);
    }

    // ========================================================================
    // Job Cancellation Tests
    // ========================================================================

    #[test]
    fn test_cancel_job_by_user() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        let escrow = 2_000_000_000_000_000_000u128;
        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                escrow,
                1000000,
            )
            .unwrap();

        let refund = market.cancel_job(job_id, user_addr, 1000001).unwrap();
        assert_eq!(refund, escrow);

        let job = market.get_job(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Cancelled);
    }

    #[test]
    fn test_cancel_job_by_provider() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        market
            .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
            .unwrap();

        // Provider cancels
        let result = market.cancel_job(job_id, provider_addr, 1000001);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cancel_job_unauthorized() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let random_addr = Address::from([99u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        // Random address tries to cancel
        let result = market.cancel_job(job_id, random_addr, 1000001);
        assert!(matches!(result, Err(ComputeMarketError::NotAuthorized)));
    }

    #[test]
    fn test_cancel_running_job_fails() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        market
            .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
            .unwrap();
        market.start_job(job_id, provider_addr, 1000001).unwrap();

        // Try to cancel running job
        let result = market.cancel_job(job_id, user_addr, 1000002);
        assert!(matches!(result, Err(ComputeMarketError::InvalidJobState)));
    }

    // ========================================================================
    // Dispute Tests
    // ========================================================================

    #[test]
    fn test_open_dispute_on_running_job() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");
        let dispute_id = Hash::hash_data(b"dispute-1");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        market
            .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
            .unwrap();
        market.start_job(job_id, provider_addr, 1000001).unwrap();

        // User opens dispute
        let result = market.open_dispute(
            dispute_id,
            job_id,
            user_addr,
            DisputeType::UnderCapacity,
            Hash::hash_data(b"evidence"),
            1000002,
        );

        assert!(result.is_ok());
        let job = market.get_job(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Disputed);
        assert_eq!(market.stats.total_jobs_disputed, 1);
    }

    #[test]
    fn test_open_dispute_unauthorized() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let random_addr = Address::from([99u8; 20]);
        let job_id = Hash::hash_data(b"test-job");
        let dispute_id = Hash::hash_data(b"dispute-1");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        market
            .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
            .unwrap();
        market.start_job(job_id, provider_addr, 1000001).unwrap();

        // Random address tries to dispute
        let result = market.open_dispute(
            dispute_id,
            job_id,
            random_addr,
            DisputeType::UnderCapacity,
            Hash::hash_data(b"evidence"),
            1000002,
        );

        assert!(matches!(result, Err(ComputeMarketError::NotAuthorized)));
    }

    #[test]
    fn test_resolve_dispute_with_slashing() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let resolver_addr = Address::from([99u8; 20]);
        let job_id = Hash::hash_data(b"test-job");
        let dispute_id = Hash::hash_data(b"dispute-1");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        let initial_stake = market.get_provider(&provider_addr).unwrap().stake;
        let escrow = 2_000_000_000_000_000_000u128;

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                escrow,
                1000000,
            )
            .unwrap();

        market
            .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
            .unwrap();
        market.start_job(job_id, provider_addr, 1000001).unwrap();

        market
            .open_dispute(
                dispute_id,
                job_id,
                user_addr,
                DisputeType::NonDelivery,
                Hash::hash_data(b"evidence"),
                1000002,
            )
            .unwrap();

        let slash_amount = 100_000_000_000_000_000_000u128; // 100 ISA
        let resolution = market
            .resolve_dispute(
                dispute_id,
                user_addr,          // User wins
                escrow,             // Full refund
                0,                  // No payment to provider
                slash_amount,       // Slash provider
                resolver_addr,
                1000003,
            )
            .unwrap();

        assert_eq!(resolution.winner, user_addr);
        assert_eq!(resolution.user_refund, escrow);
        assert_eq!(resolution.slash_amount, slash_amount);

        // Verify provider was slashed
        let provider = market.get_provider(&provider_addr).unwrap();
        assert_eq!(provider.stake, initial_stake - slash_amount);
        assert_eq!(provider.jobs_failed, 1);

        // Verify job status
        let job = market.get_job(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Failed);
    }

    #[test]
    fn test_resolve_dispute_already_resolved() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let resolver_addr = Address::from([99u8; 20]);
        let job_id = Hash::hash_data(b"test-job");
        let dispute_id = Hash::hash_data(b"dispute-1");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        market
            .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
            .unwrap();
        market.start_job(job_id, provider_addr, 1000001).unwrap();

        market
            .open_dispute(
                dispute_id,
                job_id,
                user_addr,
                DisputeType::NonDelivery,
                Hash::hash_data(b"evidence"),
                1000002,
            )
            .unwrap();

        // Resolve once
        market
            .resolve_dispute(
                dispute_id,
                user_addr,
                2_000_000_000_000_000_000,
                0,
                0,
                resolver_addr,
                1000003,
            )
            .unwrap();

        // Try to resolve again
        let result = market.resolve_dispute(
            dispute_id,
            provider_addr,
            0,
            2_000_000_000_000_000_000,
            0,
            resolver_addr,
            1000004,
        );

        assert!(matches!(
            result,
            Err(ComputeMarketError::DisputeAlreadyResolved)
        ));
    }

    // ========================================================================
    // Capacity Management Tests
    // ========================================================================

    #[test]
    fn test_capacity_deduction_on_accept() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        let initial_capacity = ComputeCapacity::new(4000, 8192, 100);
        let job_capacity = ComputeCapacity::new(1000, 2048, 10);

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                initial_capacity.clone(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                job_capacity.clone(),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        market
            .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
            .unwrap();

        // Verify capacity was deducted
        let provider = market.get_provider(&provider_addr).unwrap();
        assert_eq!(
            provider.available_capacity.cpu_millicores,
            initial_capacity.cpu_millicores - job_capacity.cpu_millicores
        );
        assert_eq!(
            provider.available_capacity.memory_mb,
            initial_capacity.memory_mb - job_capacity.memory_mb
        );
    }

    #[test]
    fn test_capacity_restoration_on_cancel() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        let initial_capacity = ComputeCapacity::new(4000, 8192, 100);
        let job_capacity = ComputeCapacity::new(1000, 2048, 10);

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                initial_capacity.clone(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                job_capacity,
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        market
            .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
            .unwrap();

        // Cancel job
        market.cancel_job(job_id, user_addr, 1000001).unwrap();

        // Verify capacity was restored
        let provider = market.get_provider(&provider_addr).unwrap();
        assert_eq!(
            provider.available_capacity.cpu_millicores,
            initial_capacity.cpu_millicores
        );
        assert_eq!(
            provider.available_capacity.memory_mb,
            initial_capacity.memory_mb
        );
    }

    #[test]
    fn test_insufficient_capacity_rejection() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);

        // Small capacity provider
        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        // Job requiring more capacity
        let result = market.create_job(
            Hash::hash_data(b"test-job"),
            user_addr,
            Some(provider_addr),
            ResourceType::VM,
            ComputeCapacity::new(8000, 16384, 100), // Way more than available
            1_000_000_000_000_000_000,
            3600,
            2_000_000_000_000_000_000,
            1000000,
        );

        assert!(matches!(
            result,
            Err(ComputeMarketError::InsufficientCapacity)
        ));
    }

    // ========================================================================
    // Query Function Tests
    // ========================================================================

    #[test]
    fn test_find_matching_providers() {
        let mut market = ComputeMarketState::new();

        // Register multiple providers with different capabilities
        let provider1 = Address::from([1u8; 20]);
        let provider2 = Address::from([2u8; 20]);
        let provider3 = Address::from([3u8; 20]);

        // Provider 1: VM, cheap
        market
            .register_provider(
                provider1,
                vec![ResourceType::VM],
                ComputeCapacity::new(4000, 8192, 100),
                500_000_000_000_000_000, // 0.5 ISA
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider1.example.com".to_string(),
            )
            .unwrap();

        // Provider 2: VM + Browser, expensive
        market
            .register_provider(
                provider2,
                vec![ResourceType::VM, ResourceType::Browser],
                ComputeCapacity::new(8000, 16384, 200),
                2_000_000_000_000_000_000, // 2 ISA
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider2.example.com".to_string(),
            )
            .unwrap();

        // Provider 3: Browser only
        market
            .register_provider(
                provider3,
                vec![ResourceType::Browser],
                ComputeCapacity::new(2000, 4096, 50),
                300_000_000_000_000_000, // 0.3 ISA
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider3.example.com".to_string(),
            )
            .unwrap();

        // Find VM providers under 1 ISA
        let matches = market.find_matching_providers(
            &ResourceType::VM,
            &ComputeCapacity::new(2000, 4096, 50),
            1_000_000_000_000_000_000,
        );

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].address, provider1);

        // Find VM providers under 3 ISA
        let matches = market.find_matching_providers(
            &ResourceType::VM,
            &ComputeCapacity::new(2000, 4096, 50),
            3_000_000_000_000_000_000,
        );

        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_list_user_jobs() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                ComputeCapacity::new(8000, 16384, 200),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        // Create multiple jobs for the same user
        for i in 0..3 {
            market
                .create_job(
                    Hash::hash_data(format!("job-{}", i).as_bytes()),
                    user_addr,
                    None,
                    ResourceType::VM,
                    ComputeCapacity::new(1000, 2048, 10),
                    1_000_000_000_000_000_000,
                    3600,
                    2_000_000_000_000_000_000,
                    1000000 + i as u64,
                )
                .unwrap();
        }

        let user_jobs = market.list_user_jobs(&user_addr);
        assert_eq!(user_jobs.len(), 3);
    }

    #[test]
    fn test_list_providers_by_resource() {
        let mut market = ComputeMarketState::new();

        // Register providers with different resource types
        for i in 0..5 {
            let addr = Address::from([i as u8; 20]);
            let resources = if i % 2 == 0 {
                vec![ResourceType::VM]
            } else {
                vec![ResourceType::Browser]
            };

            market
                .register_provider(
                    addr,
                    resources,
                    ComputeCapacity::new(4000, 8192, 100),
                    1_000_000_000_000_000_000,
                    3600,
                    86400,
                    constants::PROVIDER_MIN_STAKE,
                    None,
                    format!("https://provider{}.example.com", i),
                )
                .unwrap();
        }

        let vm_providers = market.list_providers_by_resource(&ResourceType::VM);
        assert_eq!(vm_providers.len(), 3); // 0, 2, 4

        let browser_providers = market.list_providers_by_resource(&ResourceType::Browser);
        assert_eq!(browser_providers.len(), 2); // 1, 3
    }

    #[test]
    fn test_escrow_tracking() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                ComputeCapacity::new(8000, 16384, 200),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        assert_eq!(market.total_escrow, 0);

        let escrow1 = 2_000_000_000_000_000_000u128;
        let escrow2 = 3_000_000_000_000_000_000u128;

        // Create jobs
        market
            .create_job(
                Hash::hash_data(b"job-1"),
                user_addr,
                None,
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                escrow1,
                1000000,
            )
            .unwrap();

        assert_eq!(market.total_escrow, escrow1);

        market
            .create_job(
                Hash::hash_data(b"job-2"),
                user_addr,
                None,
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                escrow2,
                1000001,
            )
            .unwrap();

        assert_eq!(market.total_escrow, escrow1 + escrow2);

        // Cancel first job
        market
            .cancel_job(Hash::hash_data(b"job-1"), user_addr, 1000002)
            .unwrap();

        assert_eq!(market.total_escrow, escrow2);
    }

    #[test]
    fn test_provider_reputation_update() {
        let mut market = ComputeMarketState::new();
        let provider_addr = Address::from([1u8; 20]);
        let user_addr = Address::from([2u8; 20]);
        let job_id = Hash::hash_data(b"test-job");

        market
            .register_provider(
                provider_addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                "https://provider.example.com".to_string(),
            )
            .unwrap();

        let initial_reputation = market.get_provider(&provider_addr).unwrap().reputation;

        market
            .create_job(
                job_id,
                user_addr,
                Some(provider_addr),
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000,
            )
            .unwrap();

        market
            .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
            .unwrap();
        market.start_job(job_id, provider_addr, 1000001).unwrap();

        market
            .settle_job(
                job_id,
                ComputeUsage {
                    duration_secs: 3600,
                    cpu_seconds: 3600,
                    memory_mb_seconds: 2048 * 3600,
                    network_bytes: 1_000_000,
                    storage_bytes: 10_000_000_000,
                },
                2000000,
            )
            .unwrap();

        // Verify reputation increased after successful job
        let final_reputation = market.get_provider(&provider_addr).unwrap().reputation;
        assert!(final_reputation > initial_reputation);
    }

    #[test]
    fn test_market_stats_accuracy() {
        let mut market = ComputeMarketState::new();

        // Register 3 providers
        for i in 0..3 {
            market
                .register_provider(
                    Address::from([i as u8; 20]),
                    vec![ResourceType::VM],
                    create_test_capacity(),
                    1_000_000_000_000_000_000,
                    3600,
                    86400,
                    constants::PROVIDER_MIN_STAKE,
                    None,
                    format!("https://provider{}.example.com", i),
                )
                .unwrap();
        }

        assert_eq!(market.stats.total_providers, 3);
        assert_eq!(market.stats.active_providers, 3);

        // Pause one
        market
            .update_provider(
                Address::from([0u8; 20]),
                None,
                None,
                Some(ProviderStatus::Paused),
                None,
                None,
            )
            .unwrap();

        assert_eq!(market.stats.total_providers, 3);
        assert_eq!(market.stats.active_providers, 2);

        // Exit one
        market.provider_exit(Address::from([1u8; 20])).unwrap();

        assert_eq!(market.stats.total_providers, 3);
        assert_eq!(market.stats.active_providers, 1);
    }
}
