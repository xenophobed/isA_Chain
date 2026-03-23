use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Simple metrics collector for Prometheus-compatible output.
///
/// Uses `AtomicU64` counters so it can be cloned and shared across threads
/// without a `Mutex`.  The `render` method accepts current chain state
/// (height, mempool size, account count) which are best read fresh from the
/// blockchain rather than tracked as counters.
#[derive(Clone)]
pub struct ChainMetrics {
    pub blocks_produced: Arc<AtomicU64>,
    pub transactions_processed: Arc<AtomicU64>,
    pub rpc_requests_total: Arc<AtomicU64>,
}

impl ChainMetrics {
    pub fn new() -> Self {
        Self {
            blocks_produced: Arc::new(AtomicU64::new(0)),
            transactions_processed: Arc::new(AtomicU64::new(0)),
            rpc_requests_total: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn inc_blocks(&self) {
        self.blocks_produced.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_transactions(&self, n: u64) {
        self.transactions_processed.fetch_add(n, Ordering::Relaxed);
    }

    pub fn inc_rpc_requests(&self) {
        self.rpc_requests_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Render Prometheus text exposition format (version 0.0.4).
    ///
    /// `chain_height`, `mempool_size`, and `account_count` are passed in
    /// rather than tracked here because they are gauge values whose ground
    /// truth lives in the `Blockchain` struct.
    pub fn render(&self, chain_height: u64, mempool_size: u64, account_count: u64) -> String {
        format!(
            "# HELP isa_chain_height Current block height\n\
             # TYPE isa_chain_height gauge\n\
             isa_chain_height {chain_height}\n\
             # HELP isa_chain_mempool_size Number of pending transactions\n\
             # TYPE isa_chain_mempool_size gauge\n\
             isa_chain_mempool_size {mempool_size}\n\
             # HELP isa_chain_account_count Number of accounts\n\
             # TYPE isa_chain_account_count gauge\n\
             isa_chain_account_count {account_count}\n\
             # HELP isa_chain_blocks_produced_total Total blocks produced since node start\n\
             # TYPE isa_chain_blocks_produced_total counter\n\
             isa_chain_blocks_produced_total {blocks_produced}\n\
             # HELP isa_chain_transactions_processed_total Total transactions processed since node start\n\
             # TYPE isa_chain_transactions_processed_total counter\n\
             isa_chain_transactions_processed_total {transactions_processed}\n\
             # HELP isa_chain_rpc_requests_total Total RPC requests served since node start\n\
             # TYPE isa_chain_rpc_requests_total counter\n\
             isa_chain_rpc_requests_total {rpc_requests_total}\n",
            chain_height = chain_height,
            mempool_size = mempool_size,
            account_count = account_count,
            blocks_produced = self.blocks_produced.load(Ordering::Relaxed),
            transactions_processed = self.transactions_processed.load(Ordering::Relaxed),
            rpc_requests_total = self.rpc_requests_total.load(Ordering::Relaxed),
        )
    }
}

impl Default for ChainMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_render_contains_all_keys() {
        let m = ChainMetrics::new();
        let output = m.render(42, 7, 100);

        assert!(output.contains("isa_chain_height 42"));
        assert!(output.contains("isa_chain_mempool_size 7"));
        assert!(output.contains("isa_chain_account_count 100"));
        assert!(output.contains("isa_chain_blocks_produced_total 0"));
        assert!(output.contains("isa_chain_transactions_processed_total 0"));
        assert!(output.contains("isa_chain_rpc_requests_total 0"));

        // Every metric must have a HELP and TYPE line
        assert_eq!(output.matches("# HELP").count(), 6);
        assert_eq!(output.matches("# TYPE").count(), 6);
    }

    #[test]
    fn test_metrics_counters_increment() {
        let m = ChainMetrics::new();

        m.inc_blocks();
        m.inc_blocks();
        m.inc_transactions(5);
        m.inc_rpc_requests();
        m.inc_rpc_requests();
        m.inc_rpc_requests();

        let output = m.render(0, 0, 0);

        assert!(output.contains("isa_chain_blocks_produced_total 2"));
        assert!(output.contains("isa_chain_transactions_processed_total 5"));
        assert!(output.contains("isa_chain_rpc_requests_total 3"));
    }

    #[test]
    fn test_metrics_clone_shares_counters() {
        let m = ChainMetrics::new();
        let m2 = m.clone();

        m.inc_blocks();
        m2.inc_blocks(); // should affect the same underlying AtomicU64

        assert_eq!(m.blocks_produced.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_metrics_render_prometheus_format() {
        let m = ChainMetrics::new();
        let output = m.render(10, 3, 50);

        // Each metric line must be "name value" with no extra whitespace issues
        for line in output.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(parts.len(), 2, "metric line should be 'name value': {line}");
            // Value must parse as a u64
            parts[1].parse::<u64>().expect("metric value should be a u64");
        }
    }
}
