use crate::quality::oracle::ProviderQualityOracle;
use crate::quality::types::{QualityMetrics, QualityReport};
use crate::subnet::SubnetId;
use crate::types::{Address, BlockHeight};

pub fn admin() -> Address {
    Address::from([0xAA; 20])
}

pub fn reporter(byte: u8) -> Address {
    Address::from([byte; 20])
}

pub fn provider(byte: u8) -> Address {
    Address::from([byte; 20])
}

pub fn metrics() -> QualityMetrics {
    QualityMetrics {
        latency_ms: 50,
        uptime_bps: 9900,
        success_rate_bps: 9800,
        throughput: 100,
    }
}

pub fn make_report(
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

pub fn oracle_with_reporter() -> (ProviderQualityOracle, Address) {
    let mut oracle = ProviderQualityOracle::new(admin(), 3);
    let r = reporter(0x01);
    oracle.authorize_reporter(r, &admin()).unwrap();
    (oracle, r)
}
