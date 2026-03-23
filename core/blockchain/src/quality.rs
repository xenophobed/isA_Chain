use crate::types::{Address, BlockHeight};
use crate::subnet::SubnetId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ============================================================================
// QualityMetrics
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Average response time in milliseconds.
    pub latency_ms: u64,
    /// Uptime in basis points (0–10000; 10000 = 100%).
    pub uptime_bps: u32,
    /// Request success rate in basis points (0–10000).
    pub success_rate_bps: u32,
    /// Requests per second.
    pub throughput: u64,
}

// ============================================================================
// QualityReport
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityReport {
    /// Provider being rated.
    pub provider: Address,
    /// Subnet the provider operates in.
    pub subnet_id: SubnetId,
    /// Address that submitted this report.
    pub reporter: Address,
    /// Composite score in basis points (0–10000).
    pub score: u32,
    /// Block at which the report was submitted.
    pub height: BlockHeight,
    /// Detailed metrics backing the score.
    pub metrics: QualityMetrics,
}

// ============================================================================
// QualityOracleError
// ============================================================================

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum QualityOracleError {
    #[error("Provider not found")]
    ProviderNotFound,

    #[error("Unauthorized reporter: {0}")]
    UnauthorizedReporter(Address),

    #[error("Invalid score: must be 0–10000 basis points")]
    InvalidScore,

    #[error("Subnet not found")]
    SubnetNotFound,

    #[error("Insufficient reports for aggregation")]
    InsufficientReports,
}

// ============================================================================
// ProviderQualityOracle
// ============================================================================

pub struct ProviderQualityOracle {
    /// All raw reports, keyed by (subnet, provider).
    pub reports: HashMap<(SubnetId, Address), Vec<QualityReport>>,
    /// Cached aggregate score per (subnet, provider).
    pub aggregated_scores: HashMap<(SubnetId, Address), u32>,
    /// Set of addresses allowed to submit reports.
    pub authorized_reporters: HashSet<Address>,
    /// Admin address — the only one who can authorize/revoke reporters.
    pub admin: Address,
    /// Minimum number of reports required before an aggregate is computed.
    pub min_reports_for_aggregate: usize,
}

impl ProviderQualityOracle {
    // ----------------------------------------------------------------
    // Constructor
    // ----------------------------------------------------------------

    pub fn new(admin: Address, min_reports: usize) -> Self {
        ProviderQualityOracle {
            reports: HashMap::new(),
            aggregated_scores: HashMap::new(),
            authorized_reporters: HashSet::new(),
            admin,
            min_reports_for_aggregate: min_reports,
        }
    }

    // ----------------------------------------------------------------
    // Reporter management
    // ----------------------------------------------------------------

    /// Authorize a new reporter.  Admin-only.
    pub fn authorize_reporter(
        &mut self,
        address: Address,
        caller: &Address,
    ) -> Result<(), QualityOracleError> {
        if caller != &self.admin {
            return Err(QualityOracleError::UnauthorizedReporter(*caller));
        }
        self.authorized_reporters.insert(address);
        Ok(())
    }

    /// Revoke an existing reporter.  Admin-only.
    pub fn revoke_reporter(
        &mut self,
        address: &Address,
        caller: &Address,
    ) -> Result<(), QualityOracleError> {
        if caller != &self.admin {
            return Err(QualityOracleError::UnauthorizedReporter(*caller));
        }
        self.authorized_reporters.remove(address);
        Ok(())
    }

    // ----------------------------------------------------------------
    // Report submission
    // ----------------------------------------------------------------

    /// Submit a quality report.
    ///
    /// Validates:
    /// - `report.reporter` is an authorized reporter
    /// - `report.score` is within 0–10000 bps
    ///
    /// After insertion the cached aggregate score for the (subnet, provider)
    /// pair is refreshed if enough reports exist.
    pub fn submit_report(&mut self, report: QualityReport) -> Result<(), QualityOracleError> {
        if !self.authorized_reporters.contains(&report.reporter) {
            return Err(QualityOracleError::UnauthorizedReporter(report.reporter));
        }
        if report.score > 10_000 {
            return Err(QualityOracleError::InvalidScore);
        }

        let key = (report.subnet_id, report.provider);
        self.reports.entry(key).or_default().push(report);

        // Eagerly refresh the cached aggregate (best-effort; ignore
        // InsufficientReports — the cached value simply stays stale until
        // enough reports accumulate).
        let _ = self.aggregate_score(&key.0, &key.1);

        Ok(())
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    /// Return the cached aggregate score for a (subnet, provider) pair, if any.
    pub fn get_score(&self, subnet_id: &SubnetId, provider: &Address) -> Option<u32> {
        self.aggregated_scores.get(&(*subnet_id, *provider)).copied()
    }

    /// Return all reports for a (subnet, provider) pair.
    pub fn get_reports(
        &self,
        subnet_id: &SubnetId,
        provider: &Address,
    ) -> Vec<&QualityReport> {
        self.reports
            .get(&(*subnet_id, *provider))
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Recalculate the aggregate score from all stored reports for the pair.
    ///
    /// Returns `Err(InsufficientReports)` when fewer than
    /// `min_reports_for_aggregate` reports exist.  On success the cached
    /// value is updated and the new score is returned.
    pub fn aggregate_score(
        &mut self,
        subnet_id: &SubnetId,
        provider: &Address,
    ) -> Result<u32, QualityOracleError> {
        let key = (*subnet_id, *provider);
        let reports = self.reports.get(&key).ok_or(QualityOracleError::ProviderNotFound)?;

        if reports.len() < self.min_reports_for_aggregate {
            return Err(QualityOracleError::InsufficientReports);
        }

        let sum: u64 = reports.iter().map(|r| r.score as u64).sum();
        let avg = (sum / reports.len() as u64) as u32;

        self.aggregated_scores.insert(key, avg);
        Ok(avg)
    }

    /// Return the top `limit` providers in a subnet, sorted by aggregate
    /// score descending.  Providers without a cached score are excluded.
    pub fn get_top_providers(
        &self,
        subnet_id: &SubnetId,
        limit: usize,
    ) -> Vec<(Address, u32)> {
        let mut entries: Vec<(Address, u32)> = self
            .aggregated_scores
            .iter()
            .filter_map(|((sid, addr), &score)| {
                if sid == subnet_id {
                    Some((*addr, score))
                } else {
                    None
                }
            })
            .collect();

        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(limit);
        entries
    }

    /// Number of distinct providers that have at least one report in `subnet_id`.
    pub fn provider_count(&self, subnet_id: &SubnetId) -> usize {
        self.reports
            .keys()
            .filter(|(sid, _)| sid == subnet_id)
            .count()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------------
    // Helpers
    // ----------------------------------------------------------------

    fn admin() -> Address {
        Address::from([0xAA; 20])
    }

    fn reporter(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    fn provider(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    fn metrics() -> QualityMetrics {
        QualityMetrics {
            latency_ms: 50,
            uptime_bps: 9900,
            success_rate_bps: 9800,
            throughput: 100,
        }
    }

    fn make_report(
        provider: Address,
        subnet_id: SubnetId,
        reporter: Address,
        score: u32,
        height: BlockHeight,
    ) -> QualityReport {
        QualityReport {
            provider,
            subnet_id,
            reporter,
            score,
            height,
            metrics: metrics(),
        }
    }

    fn oracle_with_reporter() -> (ProviderQualityOracle, Address) {
        let mut oracle = ProviderQualityOracle::new(admin(), 3);
        let r = reporter(0x01);
        oracle.authorize_reporter(r, &admin()).unwrap();
        (oracle, r)
    }

    // ----------------------------------------------------------------
    // Tests
    // ----------------------------------------------------------------

    #[test]
    fn test_submit_report() {
        let (mut oracle, r) = oracle_with_reporter();
        let p = provider(0x10);
        let report = make_report(p, SubnetId::Model, r, 8000, 1);
        oracle.submit_report(report).unwrap();
        assert_eq!(oracle.get_reports(&SubnetId::Model, &p).len(), 1);
    }

    #[test]
    fn test_unauthorized_reporter() {
        let (mut oracle, _) = oracle_with_reporter();
        let bad = reporter(0xFF);
        let p = provider(0x10);
        let report = make_report(p, SubnetId::Model, bad, 8000, 1);
        let err = oracle.submit_report(report).unwrap_err();
        assert_eq!(err, QualityOracleError::UnauthorizedReporter(bad));
    }

    #[test]
    fn test_invalid_score() {
        let (mut oracle, r) = oracle_with_reporter();
        let p = provider(0x10);
        let report = make_report(p, SubnetId::Model, r, 10_001, 1);
        let err = oracle.submit_report(report).unwrap_err();
        assert_eq!(err, QualityOracleError::InvalidScore);
    }

    #[test]
    fn test_aggregate_score() {
        let (mut oracle, r) = oracle_with_reporter();
        let p = provider(0x10);

        for (i, score) in [6000u32, 8000, 7000].iter().enumerate() {
            oracle
                .submit_report(make_report(p, SubnetId::Model, r, *score, i as u64))
                .unwrap();
        }

        // Average of 6000, 8000, 7000 = 7000
        let avg = oracle.aggregate_score(&SubnetId::Model, &p).unwrap();
        assert_eq!(avg, 7000);
        assert_eq!(oracle.get_score(&SubnetId::Model, &p), Some(7000));
    }

    #[test]
    fn test_insufficient_reports() {
        let (mut oracle, r) = oracle_with_reporter();
        let p = provider(0x10);

        // Only 2 reports; min is 3.
        oracle
            .submit_report(make_report(p, SubnetId::Model, r, 8000, 1))
            .unwrap();
        oracle
            .submit_report(make_report(p, SubnetId::Model, r, 7000, 2))
            .unwrap();

        let err = oracle.aggregate_score(&SubnetId::Model, &p).unwrap_err();
        assert_eq!(err, QualityOracleError::InsufficientReports);
    }

    #[test]
    fn test_get_top_providers() {
        let (mut oracle, r) = oracle_with_reporter();
        let p1 = provider(0x11);
        let p2 = provider(0x12);
        let p3 = provider(0x13);

        // Give each provider exactly 3 reports so aggregation succeeds.
        for (p, scores) in [
            (p1, [9000u32, 9200, 9100]),
            (p2, [7000, 6800, 7200]),
            (p3, [5000, 5200, 4800]),
        ] {
            for (i, score) in scores.iter().enumerate() {
                oracle
                    .submit_report(make_report(p, SubnetId::Tools, r, *score, i as u64))
                    .unwrap();
            }
        }

        let top = oracle.get_top_providers(&SubnetId::Tools, 2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, p1); // highest avg ≈ 9100
        assert_eq!(top[1].0, p2); // second highest avg = 7000
    }

    #[test]
    fn test_authorize_revoke() {
        let mut oracle = ProviderQualityOracle::new(admin(), 1);
        let r = reporter(0x01);

        // Not yet authorized.
        let p = provider(0x10);
        let report = make_report(p, SubnetId::Model, r, 8000, 1);
        assert_eq!(
            oracle.submit_report(report).unwrap_err(),
            QualityOracleError::UnauthorizedReporter(r)
        );

        // Authorize.
        oracle.authorize_reporter(r, &admin()).unwrap();
        oracle
            .submit_report(make_report(p, SubnetId::Model, r, 8000, 2))
            .unwrap();

        // Revoke.
        oracle.revoke_reporter(&r, &admin()).unwrap();
        assert_eq!(
            oracle
                .submit_report(make_report(p, SubnetId::Model, r, 8000, 3))
                .unwrap_err(),
            QualityOracleError::UnauthorizedReporter(r)
        );
    }

    #[test]
    fn test_authorize_requires_admin() {
        let mut oracle = ProviderQualityOracle::new(admin(), 1);
        let bad = reporter(0xBB);
        let r = reporter(0x01);
        let err = oracle.authorize_reporter(r, &bad).unwrap_err();
        assert_eq!(err, QualityOracleError::UnauthorizedReporter(bad));
    }

    #[test]
    fn test_revoke_requires_admin() {
        let mut oracle = ProviderQualityOracle::new(admin(), 1);
        let r = reporter(0x01);
        oracle.authorize_reporter(r, &admin()).unwrap();
        let bad = reporter(0xBB);
        let err = oracle.revoke_reporter(&r, &bad).unwrap_err();
        assert_eq!(err, QualityOracleError::UnauthorizedReporter(bad));
    }

    #[test]
    fn test_multiple_subnets() {
        let (mut oracle, r) = oracle_with_reporter();
        let p = provider(0x10);

        // Same provider, two different subnets.
        for i in 0..3 {
            oracle
                .submit_report(make_report(p, SubnetId::Model, r, 9000, i))
                .unwrap();
            oracle
                .submit_report(make_report(p, SubnetId::Storage, r, 5000, i))
                .unwrap();
        }

        assert_eq!(oracle.aggregate_score(&SubnetId::Model, &p).unwrap(), 9000);
        assert_eq!(
            oracle.aggregate_score(&SubnetId::Storage, &p).unwrap(),
            5000
        );

        // Scores don't bleed across subnets.
        assert_ne!(
            oracle.get_score(&SubnetId::Model, &p),
            oracle.get_score(&SubnetId::Storage, &p)
        );
    }

    #[test]
    fn test_provider_count() {
        let (mut oracle, r) = oracle_with_reporter();
        assert_eq!(oracle.provider_count(&SubnetId::Agent), 0);

        let p1 = provider(0x11);
        let p2 = provider(0x12);

        oracle
            .submit_report(make_report(p1, SubnetId::Agent, r, 7000, 1))
            .unwrap();
        assert_eq!(oracle.provider_count(&SubnetId::Agent), 1);

        oracle
            .submit_report(make_report(p2, SubnetId::Agent, r, 8000, 2))
            .unwrap();
        assert_eq!(oracle.provider_count(&SubnetId::Agent), 2);

        // Different subnet — should not affect Agent count.
        oracle
            .submit_report(make_report(p1, SubnetId::Model, r, 9000, 3))
            .unwrap();
        assert_eq!(oracle.provider_count(&SubnetId::Agent), 2);
    }

    #[test]
    fn test_get_reports() {
        let (mut oracle, r) = oracle_with_reporter();
        let p = provider(0x10);

        assert!(oracle.get_reports(&SubnetId::Compute, &p).is_empty());

        oracle
            .submit_report(make_report(p, SubnetId::Compute, r, 7500, 1))
            .unwrap();
        oracle
            .submit_report(make_report(p, SubnetId::Compute, r, 8000, 2))
            .unwrap();

        let reports = oracle.get_reports(&SubnetId::Compute, &p);
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].score, 7500);
        assert_eq!(reports[1].score, 8000);
    }
}
