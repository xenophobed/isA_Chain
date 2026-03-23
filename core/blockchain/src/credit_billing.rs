use crate::settlement::ServiceType;
use crate::types::{Address, Amount, BlockHeight, Hash, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ============================================================================
// CreditBillingRecord
// ============================================================================

/// A single credit-billing event for a user consuming a service.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreditBillingRecord {
    /// Unique identifier for this billing record
    pub id: Hash,
    /// User address that was charged
    pub user: Address,
    /// Which service was consumed
    pub service: ServiceType,
    /// Number of credits deducted
    pub credits_charged: Amount,
    /// Human-readable description of the usage
    pub description: String,
    /// Block height at which this charge occurred
    pub height: BlockHeight,
    /// Unix timestamp (ms) at which this charge occurred
    pub timestamp: Timestamp,
}

// ============================================================================
// ServiceUsageSummary
// ============================================================================

/// Aggregate usage statistics for a single service type.
#[derive(Clone, Debug)]
pub struct ServiceUsageSummary {
    /// The service this summary covers
    pub service: ServiceType,
    /// Total credits charged across all users
    pub total_credits: Amount,
    /// Total number of billing requests
    pub total_requests: u64,
    /// Average credits per request (0 when no requests)
    pub average_cost: Amount,
}

// ============================================================================
// CreditBillingError
// ============================================================================

/// Errors that can occur during credit billing operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CreditBillingError {
    #[error("Insufficient credits: available {available}, required {required}")]
    InsufficientCredits { available: Amount, required: Amount },

    #[error("Service is not enabled for billing")]
    ServiceNotEnabled,

    #[error("Rate limit exceeded for user/service combination")]
    RateLimitExceeded,

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount,

    #[error("User not found: {0:?}")]
    UserNotFound(Address),
}

// ============================================================================
// CreditBillingEngine
// ============================================================================

/// Middleware engine that tracks per-service credit consumption and enforces
/// spending limits for isA_Model and isA_MCP services.
///
/// ## Rate limiting
///
/// Requests are counted per `(user, service)` pair within rolling windows of
/// `period_length` blocks.  When `current_height >= period_start + period_length`
/// the counter resets.
pub struct CreditBillingEngine {
    /// Ordered list of all billing records
    pub records: Vec<CreditBillingRecord>,
    /// Index: user → indices into `records`
    pub by_user: HashMap<Address, Vec<usize>>,
    /// Index: service → indices into `records`
    pub by_service: HashMap<ServiceType, Vec<usize>>,
    /// Total credits charged per user over all time
    pub user_totals: HashMap<Address, Amount>,
    /// `(total_credits, request_count)` per service over all time
    pub service_totals: HashMap<ServiceType, (Amount, u64)>,
    /// Set of services currently accepting charges
    pub enabled_services: HashSet<ServiceType>,
    /// Rate-limit state: `(user, service)` → `(request_count, period_start_height)`
    pub rate_limits: HashMap<(Address, ServiceType), (u64, BlockHeight)>,
    /// Maximum requests allowed per `(user, service)` per period
    pub max_requests_per_period: u64,
    /// Number of blocks in one rate-limit period
    pub period_length: u64,
}

impl CreditBillingEngine {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Create a new engine.
    ///
    /// - `max_requests`: maximum requests per `(user, service)` per period (default 1 000)
    /// - `period_length`: number of blocks in one period (default 100)
    pub fn new(max_requests: u64, period_length: u64) -> Self {
        CreditBillingEngine {
            records: Vec::new(),
            by_user: HashMap::new(),
            by_service: HashMap::new(),
            user_totals: HashMap::new(),
            service_totals: HashMap::new(),
            enabled_services: HashSet::new(),
            rate_limits: HashMap::new(),
            max_requests_per_period: max_requests,
            period_length,
        }
    }

    // -----------------------------------------------------------------------
    // Service management
    // -----------------------------------------------------------------------

    /// Enable a service so that it can accept charges.
    pub fn enable_service(&mut self, service: ServiceType) {
        self.enabled_services.insert(service);
    }

    /// Disable a service; future charge attempts for it will return
    /// [`CreditBillingError::ServiceNotEnabled`].
    pub fn disable_service(&mut self, service: &ServiceType) {
        self.enabled_services.remove(service);
    }

    // -----------------------------------------------------------------------
    // Core billing
    // -----------------------------------------------------------------------

    /// Charge `credits` from a user's balance for consuming `service`.
    ///
    /// Validates:
    /// 1. `credits > 0` (returns [`CreditBillingError::InvalidAmount`])
    /// 2. Service is enabled (returns [`CreditBillingError::ServiceNotEnabled`])
    /// 3. Rate limit not exceeded (returns [`CreditBillingError::RateLimitExceeded`])
    /// 4. `credit_balance >= credits` (returns [`CreditBillingError::InsufficientCredits`])
    ///
    /// On success, records the billing event and updates all indexes/totals.
    pub fn charge(
        &mut self,
        user: Address,
        service: ServiceType,
        credits: Amount,
        description: String,
        credit_balance: Amount,
        height: BlockHeight,
        timestamp: Timestamp,
    ) -> Result<CreditBillingRecord, CreditBillingError> {
        if credits == 0 {
            return Err(CreditBillingError::InvalidAmount);
        }

        if !self.enabled_services.contains(&service) {
            return Err(CreditBillingError::ServiceNotEnabled);
        }

        if !self.check_rate_limit(&user, &service, height) {
            return Err(CreditBillingError::RateLimitExceeded);
        }

        if credit_balance < credits {
            return Err(CreditBillingError::InsufficientCredits {
                available: credit_balance,
                required: credits,
            });
        }

        // Bump the rate-limit counter for this (user, service) pair.
        let key = (user, service.clone());
        let entry = self.rate_limits.entry(key).or_insert((0, height));
        // Reset if we've moved into a new period.
        if height >= entry.1 + self.period_length {
            *entry = (0, height);
        }
        entry.0 += 1;

        // Build a deterministic record id from the record index + user + height.
        let record_index = self.records.len();
        let mut id_data = Vec::with_capacity(52);
        id_data.extend_from_slice(user.as_bytes());
        id_data.extend_from_slice(&height.to_le_bytes());
        id_data.extend_from_slice(&(record_index as u64).to_le_bytes());
        let id = Hash::hash_data(&id_data);

        let record = CreditBillingRecord {
            id,
            user,
            service: service.clone(),
            credits_charged: credits,
            description,
            height,
            timestamp,
        };

        // Update indexes.
        self.by_user
            .entry(user)
            .or_default()
            .push(record_index);
        self.by_service
            .entry(service.clone())
            .or_default()
            .push(record_index);

        // Update totals.
        *self.user_totals.entry(user).or_insert(0) += credits;
        let svc_entry = self
            .service_totals
            .entry(service)
            .or_insert((0, 0));
        svc_entry.0 += credits;
        svc_entry.1 += 1;

        self.records.push(record.clone());

        Ok(record)
    }

    // -----------------------------------------------------------------------
    // Queries — users
    // -----------------------------------------------------------------------

    /// Total credits charged to `user` across all services and all time.
    ///
    /// Returns 0 when the user has never been charged.
    pub fn get_user_total(&self, user: &Address) -> Amount {
        self.user_totals.get(user).copied().unwrap_or(0)
    }

    /// All billing records for `user`, in insertion order.
    pub fn get_user_records(&self, user: &Address) -> Vec<&CreditBillingRecord> {
        self.by_user
            .get(user)
            .map(|indices| indices.iter().map(|&i| &self.records[i]).collect())
            .unwrap_or_default()
    }

    // -----------------------------------------------------------------------
    // Queries — services
    // -----------------------------------------------------------------------

    /// Usage summary for `service`, or `None` if it has never been charged.
    pub fn get_service_summary(&self, service: &ServiceType) -> Option<ServiceUsageSummary> {
        self.service_totals.get(service).map(|&(total, count)| {
            let average = if count > 0 { total / count as u128 } else { 0 };
            ServiceUsageSummary {
                service: service.clone(),
                total_credits: total,
                total_requests: count,
                average_cost: average,
            }
        })
    }

    /// Summaries for every service that has at least one billing record.
    pub fn get_all_service_summaries(&self) -> Vec<ServiceUsageSummary> {
        self.service_totals
            .iter()
            .map(|(svc, &(total, count))| {
                let average = if count > 0 { total / count as u128 } else { 0 };
                ServiceUsageSummary {
                    service: svc.clone(),
                    total_credits: total,
                    total_requests: count,
                    average_cost: average,
                }
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Rate limiting
    // -----------------------------------------------------------------------

    /// Return `true` when the user is still within their rate limit for this
    /// service at `current_height`.
    ///
    /// A new period begins whenever `current_height >= period_start + period_length`.
    pub fn check_rate_limit(
        &self,
        user: &Address,
        service: &ServiceType,
        current_height: BlockHeight,
    ) -> bool {
        let key = (*user, service.clone());
        match self.rate_limits.get(&key) {
            None => true, // no history — within limit
            Some(&(count, period_start)) => {
                if current_height >= period_start + self.period_length {
                    // Period has elapsed — counter would reset, so within limit.
                    true
                } else {
                    count < self.max_requests_per_period
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Global stats
    // -----------------------------------------------------------------------

    /// Sum of all credits ever charged across all users and services.
    pub fn total_credits_charged(&self) -> Amount {
        self.user_totals.values().sum()
    }

    /// Total number of billing records stored.
    pub fn total_records(&self) -> usize {
        self.records.len()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Fixtures
    // -----------------------------------------------------------------------

    fn user1() -> Address {
        Address::from([0x01; 20])
    }

    fn user2() -> Address {
        Address::from([0x02; 20])
    }

    fn service_model() -> ServiceType {
        ServiceType::ModelInference
    }

    fn service_tool() -> ServiceType {
        ServiceType::ToolExecution
    }

    /// Build a default engine with ModelInference and ToolExecution enabled,
    /// max 10 requests per period, period = 100 blocks.
    fn setup() -> CreditBillingEngine {
        let mut engine = CreditBillingEngine::new(10, 100);
        engine.enable_service(service_model());
        engine.enable_service(service_tool());
        engine
    }

    // -----------------------------------------------------------------------
    // test_charge_success
    // -----------------------------------------------------------------------

    #[test]
    fn test_charge_success() {
        let mut engine = setup();
        let record = engine
            .charge(user1(), service_model(), 50, "inference".into(), 1_000, 1, 1_000)
            .unwrap();

        assert_eq!(record.user, user1());
        assert_eq!(record.service, service_model());
        assert_eq!(record.credits_charged, 50);
        assert_eq!(record.height, 1);
        assert_eq!(record.timestamp, 1_000);
        assert_eq!(engine.total_records(), 1);
    }

    // -----------------------------------------------------------------------
    // test_insufficient_credits
    // -----------------------------------------------------------------------

    #[test]
    fn test_insufficient_credits() {
        let mut engine = setup();
        let err = engine
            .charge(user1(), service_model(), 200, "inference".into(), 100, 1, 1_000)
            .unwrap_err();

        assert_eq!(
            err,
            CreditBillingError::InsufficientCredits {
                available: 100,
                required: 200
            }
        );
        assert_eq!(engine.total_records(), 0);
    }

    // -----------------------------------------------------------------------
    // test_service_not_enabled
    // -----------------------------------------------------------------------

    #[test]
    fn test_service_not_enabled() {
        let mut engine = setup();
        let err = engine
            .charge(
                user1(),
                ServiceType::Storage,
                10,
                "storage".into(),
                1_000,
                1,
                1_000,
            )
            .unwrap_err();

        assert_eq!(err, CreditBillingError::ServiceNotEnabled);
    }

    // -----------------------------------------------------------------------
    // test_rate_limit
    // -----------------------------------------------------------------------

    #[test]
    fn test_rate_limit() {
        // Engine with max 3 requests per period of 100 blocks.
        let mut engine = CreditBillingEngine::new(3, 100);
        engine.enable_service(service_model());

        for i in 0..3u64 {
            engine
                .charge(user1(), service_model(), 1, "req".into(), 1_000, i, 0)
                .expect("should succeed");
        }

        // 4th request in same period should fail.
        let err = engine
            .charge(user1(), service_model(), 1, "req".into(), 1_000, 3, 0)
            .unwrap_err();
        assert_eq!(err, CreditBillingError::RateLimitExceeded);
    }

    // -----------------------------------------------------------------------
    // test_rate_limit_reset
    // -----------------------------------------------------------------------

    #[test]
    fn test_rate_limit_reset() {
        let mut engine = CreditBillingEngine::new(2, 100);
        engine.enable_service(service_model());

        // Exhaust the limit in period starting at block 0.
        engine
            .charge(user1(), service_model(), 1, "r1".into(), 1_000, 0, 0)
            .unwrap();
        engine
            .charge(user1(), service_model(), 1, "r2".into(), 1_000, 1, 0)
            .unwrap();

        // Verify limit is hit.
        assert_eq!(
            engine.charge(user1(), service_model(), 1, "r3".into(), 1_000, 2, 0).unwrap_err(),
            CreditBillingError::RateLimitExceeded
        );

        // Move to a new period (block 100+).
        let record = engine
            .charge(user1(), service_model(), 1, "r4".into(), 1_000, 100, 0)
            .unwrap();
        assert_eq!(record.height, 100);
    }

    // -----------------------------------------------------------------------
    // test_user_total
    // -----------------------------------------------------------------------

    #[test]
    fn test_user_total() {
        let mut engine = setup();

        engine
            .charge(user1(), service_model(), 30, "a".into(), 500, 1, 0)
            .unwrap();
        engine
            .charge(user1(), service_tool(), 20, "b".into(), 470, 2, 0)
            .unwrap();

        assert_eq!(engine.get_user_total(&user1()), 50);
        assert_eq!(engine.get_user_total(&user2()), 0);
    }

    // -----------------------------------------------------------------------
    // test_service_summary
    // -----------------------------------------------------------------------

    #[test]
    fn test_service_summary() {
        let mut engine = setup();

        engine
            .charge(user1(), service_model(), 40, "x".into(), 500, 1, 0)
            .unwrap();
        engine
            .charge(user2(), service_model(), 60, "y".into(), 500, 2, 0)
            .unwrap();

        let summary = engine.get_service_summary(&service_model()).unwrap();
        assert_eq!(summary.total_credits, 100);
        assert_eq!(summary.total_requests, 2);
        assert_eq!(summary.average_cost, 50);

        // Unknown service returns None.
        assert!(engine.get_service_summary(&ServiceType::Storage).is_none());
    }

    // -----------------------------------------------------------------------
    // test_all_summaries
    // -----------------------------------------------------------------------

    #[test]
    fn test_all_summaries() {
        let mut engine = setup();

        engine
            .charge(user1(), service_model(), 10, "m".into(), 500, 1, 0)
            .unwrap();
        engine
            .charge(user1(), service_tool(), 20, "t".into(), 490, 2, 0)
            .unwrap();

        let summaries = engine.get_all_service_summaries();
        assert_eq!(summaries.len(), 2);

        let model_sum: Amount = summaries
            .iter()
            .find(|s| s.service == service_model())
            .map(|s| s.total_credits)
            .unwrap();
        let tool_sum: Amount = summaries
            .iter()
            .find(|s| s.service == service_tool())
            .map(|s| s.total_credits)
            .unwrap();

        assert_eq!(model_sum, 10);
        assert_eq!(tool_sum, 20);
    }

    // -----------------------------------------------------------------------
    // test_multiple_users
    // -----------------------------------------------------------------------

    #[test]
    fn test_multiple_users() {
        let mut engine = setup();

        engine
            .charge(user1(), service_model(), 15, "u1".into(), 500, 1, 0)
            .unwrap();
        engine
            .charge(user2(), service_model(), 25, "u2".into(), 500, 2, 0)
            .unwrap();

        assert_eq!(engine.get_user_total(&user1()), 15);
        assert_eq!(engine.get_user_total(&user2()), 25);
        assert_eq!(engine.get_user_records(&user1()).len(), 1);
        assert_eq!(engine.get_user_records(&user2()).len(), 1);
    }

    // -----------------------------------------------------------------------
    // test_multiple_services
    // -----------------------------------------------------------------------

    #[test]
    fn test_multiple_services() {
        let mut engine = setup();

        engine
            .charge(user1(), service_model(), 10, "m".into(), 500, 1, 0)
            .unwrap();
        engine
            .charge(user1(), service_tool(), 5, "t".into(), 490, 2, 0)
            .unwrap();

        assert_eq!(engine.get_service_summary(&service_model()).unwrap().total_credits, 10);
        assert_eq!(engine.get_service_summary(&service_tool()).unwrap().total_credits, 5);
        assert_eq!(engine.total_records(), 2);
    }

    // -----------------------------------------------------------------------
    // test_total_tracking
    // -----------------------------------------------------------------------

    #[test]
    fn test_total_tracking() {
        let mut engine = setup();

        engine
            .charge(user1(), service_model(), 100, "a".into(), 500, 1, 0)
            .unwrap();
        engine
            .charge(user2(), service_tool(), 200, "b".into(), 500, 2, 0)
            .unwrap();

        assert_eq!(engine.total_credits_charged(), 300);
        assert_eq!(engine.total_records(), 2);
    }

    // -----------------------------------------------------------------------
    // test_invalid_amount
    // -----------------------------------------------------------------------

    #[test]
    fn test_invalid_amount() {
        let mut engine = setup();
        let err = engine
            .charge(user1(), service_model(), 0, "zero".into(), 500, 1, 0)
            .unwrap_err();
        assert_eq!(err, CreditBillingError::InvalidAmount);
    }

    // -----------------------------------------------------------------------
    // test_disable_service
    // -----------------------------------------------------------------------

    #[test]
    fn test_disable_service() {
        let mut engine = setup();

        // Works before disable.
        engine
            .charge(user1(), service_model(), 10, "ok".into(), 500, 1, 0)
            .unwrap();

        engine.disable_service(&service_model());

        let err = engine
            .charge(user1(), service_model(), 10, "fail".into(), 500, 2, 0)
            .unwrap_err();
        assert_eq!(err, CreditBillingError::ServiceNotEnabled);
    }
}
