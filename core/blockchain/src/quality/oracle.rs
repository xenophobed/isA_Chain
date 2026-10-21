use super::types::*;
use crate::subnet::SubnetId;
use crate::types::Address;
use std::collections::{HashMap, HashSet};

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
