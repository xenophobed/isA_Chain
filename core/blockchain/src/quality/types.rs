use crate::subnet::SubnetId;
use crate::types::{Address, BlockHeight};
use serde::{Deserialize, Serialize};

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
