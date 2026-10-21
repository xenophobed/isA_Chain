//! Load / stress tests for the isA_Chain core blockchain crate.
//!
//! These tests exercise the chain under high transaction volumes, deep block
//! histories, large account sets, and full economy cycles, all without any
//! network or async dependency.
//!
//! Run with:
//!   cargo test --manifest-path core/blockchain/Cargo.toml --test load_test 2>&1 | tail -30

mod economy_stress_tests;
mod helpers;
mod mempool_account_tests;
mod throughput_tests;
