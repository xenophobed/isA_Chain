//! Compute Subnet — Bridge between ComputeMarketplace and SubnetRegistry
//!
//! This module activates the existing ComputeMarketplace as the canonical
//! Compute subnet (`SubnetId::Compute`) within the isA subnet architecture.

use crate::compute_market::{ComputeMarketError, ComputeMarketState};
use crate::subnet::SubnetId;
use crate::types::{Address, Amount, ComputeCapacity, ComputeUsage, Hash, ResourceType, Timestamp};

// ============================================================================
// ComputeSubnetError
// ============================================================================

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ComputeSubnetError {
    #[error("Market error: {0}")]
    MarketError(String),

    #[error("Provider not registered: {0:?}")]
    ProviderNotRegistered(Address),

    #[error("Subnet is not active")]
    SubnetNotActive,

    #[error("Insufficient stake to register as compute provider")]
    InsufficientStake,
}

impl From<ComputeMarketError> for ComputeSubnetError {
    fn from(e: ComputeMarketError) -> Self {
        match e {
            ComputeMarketError::ProviderNotFound => {
                // Re-map to ProviderNotRegistered with a zero address as sentinel;
                // callers that need the address should check before calling.
                ComputeSubnetError::MarketError(e.to_string())
            }
            ComputeMarketError::InsufficientStake => ComputeSubnetError::InsufficientStake,
            other => ComputeSubnetError::MarketError(other.to_string()),
        }
    }
}

// ============================================================================
// ComputeSubnet
// ============================================================================

/// The Compute subnet — wraps `ComputeMarketState` and exposes it as
/// `SubnetId::Compute` within the isA subnet architecture.
pub struct ComputeSubnet {
    /// Always `SubnetId::Compute`.
    pub subnet_id: SubnetId,
    /// Underlying marketplace state.
    pub market: ComputeMarketState,
    /// Running total of jobs that have been settled.
    pub total_jobs_settled: u64,
    /// Cumulative settlement revenue (provider payments + protocol fees).
    pub total_revenue: Amount,
    /// Cache of distinct provider addresses currently registered.
    pub active_providers: usize,
}

impl ComputeSubnet {
    // ----------------------------------------------------------------
    // Constructor
    // ----------------------------------------------------------------

    /// Create a fresh Compute subnet backed by an empty marketplace.
    pub fn new() -> Self {
        ComputeSubnet {
            subnet_id: SubnetId::Compute,
            market: ComputeMarketState::new(),
            total_jobs_settled: 0,
            total_revenue: 0,
            active_providers: 0,
        }
    }

    // ----------------------------------------------------------------
    // Provider Operations
    // ----------------------------------------------------------------

    /// Register a compute provider in the subnet.
    ///
    /// Uses sensible defaults for `min_duration_secs` (1 h) and
    /// `max_duration_secs` (24 h) so callers only need to supply the
    /// public-facing fields described in the issue spec.
    pub fn register_provider(
        &mut self,
        address: Address,
        capacity: ComputeCapacity,
        price_per_hour: Amount,
        endpoint: String,
    ) -> Result<(), ComputeSubnetError> {
        // Use a stake equal to the protocol minimum (PROVIDER_MIN_STAKE).
        use crate::types::constants::PROVIDER_MIN_STAKE;

        self.market
            .register_provider(
                address,
                vec![
                    ResourceType::VM,
                    ResourceType::Browser,
                    ResourceType::REPL,
                ],
                capacity,
                price_per_hour,
                3_600,   // min_duration_secs — 1 hour
                86_400,  // max_duration_secs — 24 hours
                PROVIDER_MIN_STAKE,
                None,    // region
                endpoint,
            )
            .map_err(ComputeSubnetError::from)?;

        self.active_providers += 1;
        Ok(())
    }

    // ----------------------------------------------------------------
    // Job Operations
    // ----------------------------------------------------------------

    /// Submit a compute job to the marketplace.
    ///
    /// The `escrow_amount` is derived from `max_price * duration_secs / 3600`
    /// so callers don't need to compute it separately.
    pub fn submit_job(
        &mut self,
        job_id: Hash,
        user: Address,
        provider: Option<Address>,
        resource_type: ResourceType,
        capacity: ComputeCapacity,
        max_price: Amount,
        duration_secs: u64,
        timestamp: Timestamp,
    ) -> Result<(), ComputeSubnetError> {
        // Verify any explicitly-requested provider is registered.
        if let Some(prov_addr) = provider {
            if self.market.get_provider(&prov_addr).is_none() {
                return Err(ComputeSubnetError::ProviderNotRegistered(prov_addr));
            }
        }

        // Escrow = max_price_per_hour * (duration_secs / 3600), rounded up.
        let duration_hours_ceil = duration_secs.saturating_add(3599) / 3600;
        let escrow_amount = max_price.saturating_mul(duration_hours_ceil as u128);

        self.market
            .create_job(
                job_id,
                user,
                provider,
                resource_type,
                capacity,
                max_price,
                duration_secs,
                escrow_amount,
                timestamp,
            )
            .map_err(ComputeSubnetError::from)
    }

    /// Complete a job and settle payment.
    ///
    /// Before calling this method the job must have been accepted
    /// (`accept_job`) and started (`start_job`) via the underlying market.
    /// Returns the total settlement amount paid to the provider.
    pub fn complete_job(
        &mut self,
        job_id: &Hash,
        usage: ComputeUsage,
        timestamp: Timestamp,
    ) -> Result<Amount, ComputeSubnetError> {
        let result = self
            .market
            .settle_job(*job_id, usage, timestamp)
            .map_err(ComputeSubnetError::from)?;

        self.total_jobs_settled += 1;
        self.total_revenue = self
            .total_revenue
            .saturating_add(result.provider_payment)
            .saturating_add(result.protocol_fee);

        Ok(result.provider_payment)
    }

    // ----------------------------------------------------------------
    // Query Methods
    // ----------------------------------------------------------------

    /// Number of providers currently registered in this subnet.
    pub fn get_provider_count(&self) -> usize {
        self.active_providers
    }

    /// Number of jobs currently tracked in the market (all states).
    pub fn get_active_jobs(&self) -> usize {
        self.market.jobs.len()
    }

    /// Cumulative revenue settled through this subnet.
    pub fn get_total_revenue(&self) -> Amount {
        self.total_revenue
    }

    /// This subnet's identifier — always `SubnetId::Compute`.
    pub fn get_subnet_id(&self) -> SubnetId {
        self.subnet_id
    }

    /// Borrow the underlying marketplace state for read-only inspection.
    pub fn get_market(&self) -> &ComputeMarketState {
        &self.market
    }
}

impl Default for ComputeSubnet {
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
    use crate::types::{ComputeCapacity, ComputeUsage, JobStatus};

    // ---- helpers -----------------------------------------------------------

    fn addr(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    fn job_id(byte: u8) -> Hash {
        Hash::from([byte; 32])
    }

    fn capacity() -> ComputeCapacity {
        ComputeCapacity::standard_vm()
    }

    const PRICE: Amount = 1_000_000_000_000_000_000; // 1 ISA / hour

    /// Register one provider and return a fresh subnet.
    fn subnet_with_provider() -> ComputeSubnet {
        let mut s = ComputeSubnet::new();
        s.register_provider(addr(0x01), capacity(), PRICE, "http://p1.local".to_string())
            .expect("register_provider should succeed");
        s
    }

    // ---- tests -------------------------------------------------------------

    #[test]
    fn test_get_subnet_id() {
        let s = ComputeSubnet::new();
        assert_eq!(s.get_subnet_id(), SubnetId::Compute);
    }

    #[test]
    fn test_register_provider() {
        let mut s = ComputeSubnet::new();
        assert_eq!(s.get_provider_count(), 0);

        s.register_provider(addr(0x01), capacity(), PRICE, "http://p1.local".to_string())
            .expect("first registration should succeed");

        assert_eq!(s.get_provider_count(), 1);
        assert!(s.get_market().get_provider(&addr(0x01)).is_some());
    }

    #[test]
    fn test_provider_count() {
        let mut s = ComputeSubnet::new();
        for i in 1u8..=3 {
            s.register_provider(
                addr(i),
                capacity(),
                PRICE,
                format!("http://p{i}.local"),
            )
            .expect("registration should succeed");
        }
        assert_eq!(s.get_provider_count(), 3);
    }

    #[test]
    fn test_submit_job() {
        let mut s = subnet_with_provider();

        s.submit_job(
            job_id(0xA0),
            addr(0xBB),         // user
            Some(addr(0x01)),   // provider
            ResourceType::VM,
            capacity(),
            PRICE,
            3600,               // 1 hour
            1_000_000,          // timestamp
        )
        .expect("submit_job should succeed");

        assert_eq!(s.get_active_jobs(), 1);
    }

    #[test]
    fn test_unregistered_provider() {
        let mut s = ComputeSubnet::new();

        let err = s
            .submit_job(
                job_id(0xA1),
                addr(0xBB),
                Some(addr(0xFF)), // not registered
                ResourceType::VM,
                capacity(),
                PRICE,
                3600,
                1_000_000,
            )
            .unwrap_err();

        assert_eq!(err, ComputeSubnetError::ProviderNotRegistered(addr(0xFF)));
    }

    #[test]
    fn test_complete_job() {
        let mut s = subnet_with_provider();
        let jid = job_id(0xA2);
        let provider_addr = addr(0x01);
        let ts: Timestamp = 1_000_000;

        // Create → Accept → Start → Settle
        s.submit_job(
            jid,
            addr(0xBB),
            Some(provider_addr),
            ResourceType::VM,
            capacity(),
            PRICE,
            3600,
            ts,
        )
        .expect("submit_job");

        s.market
            .accept_job(jid, provider_addr, PRICE)
            .expect("accept_job");

        s.market
            .start_job(jid, provider_addr, ts + 1)
            .expect("start_job");

        let usage = ComputeUsage {
            duration_secs: 3600,
            cpu_seconds: 3600,
            memory_mb_seconds: 1024 * 3600,
            storage_bytes: 0,
            network_bytes: 0,
        };

        let payment = s
            .complete_job(&jid, usage, ts + 3601)
            .expect("complete_job");

        assert!(payment > 0, "provider payment should be positive");
        assert_eq!(s.total_jobs_settled, 1);
    }

    #[test]
    fn test_revenue_tracking() {
        let mut s = subnet_with_provider();
        let provider_addr = addr(0x01);

        // Run two jobs end-to-end and verify revenue accumulates.
        for (i, byte) in [0xB0u8, 0xB1u8].iter().enumerate() {
            let jid = job_id(*byte);
            let ts: Timestamp = 1_000_000 + i as u64 * 100_000;

            s.submit_job(
                jid,
                addr(0xCC),
                Some(provider_addr),
                ResourceType::VM,
                capacity(),
                PRICE,
                3600,
                ts,
            )
            .unwrap();

            s.market.accept_job(jid, provider_addr, PRICE).unwrap();
            s.market.start_job(jid, provider_addr, ts + 1).unwrap();

            let usage = ComputeUsage {
                duration_secs: 3600,
                cpu_seconds: 3600,
                memory_mb_seconds: 512 * 3600,
                storage_bytes: 0,
                network_bytes: 0,
            };

            s.complete_job(&jid, usage, ts + 3601).unwrap();
        }

        assert_eq!(s.total_jobs_settled, 2);
        assert!(s.get_total_revenue() > 0);
    }

    #[test]
    fn test_job_lifecycle() {
        let mut s = subnet_with_provider();
        let jid = job_id(0xC0);
        let provider_addr = addr(0x01);
        let ts: Timestamp = 2_000_000;

        // Pending after submit
        s.submit_job(
            jid,
            addr(0xDD),
            Some(provider_addr),
            ResourceType::VM,
            capacity(),
            PRICE,
            7200,
            ts,
        )
        .unwrap();
        assert_eq!(
            s.get_market().get_job(&jid).unwrap().status,
            JobStatus::Pending
        );

        // Matched after accept
        s.market.accept_job(jid, provider_addr, PRICE).unwrap();
        assert_eq!(
            s.get_market().get_job(&jid).unwrap().status,
            JobStatus::Matched
        );

        // Running after start
        s.market.start_job(jid, provider_addr, ts + 1).unwrap();
        assert_eq!(
            s.get_market().get_job(&jid).unwrap().status,
            JobStatus::Running
        );

        // Completed after settle
        let usage = ComputeUsage {
            duration_secs: 7200,
            cpu_seconds: 7200,
            memory_mb_seconds: 2048 * 7200,
            storage_bytes: 10_000_000_000,
            network_bytes: 52_428_800,
        };
        s.complete_job(&jid, usage, ts + 7201).unwrap();
        assert_eq!(
            s.get_market().get_job(&jid).unwrap().status,
            JobStatus::Completed
        );
    }
}
