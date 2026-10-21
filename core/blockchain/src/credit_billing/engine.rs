use crate::settlement::ServiceType;
use crate::types::{Address, Amount, BlockHeight, Hash, Timestamp};
use std::collections::{HashMap, HashSet};

use super::types::{CreditBillingError, CreditBillingRecord, ServiceUsageSummary};

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
    #[allow(clippy::too_many_arguments)]
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
