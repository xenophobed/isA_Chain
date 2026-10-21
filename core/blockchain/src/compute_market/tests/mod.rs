use crate::types::ComputeCapacity;

pub fn create_test_capacity() -> ComputeCapacity {
    ComputeCapacity::new(4000, 8192, 100)
}

mod provider_tests;
mod job_create_tests;
mod job_lifecycle_tests;
mod settlement_tests;
mod dispute_tests;
mod capacity_query_tests;
mod stats_escrow_tests;
