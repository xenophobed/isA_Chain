//! Smoke / integration tests for the isA_Chain core blockchain crate.
//!
//! These tests exercise end-to-end flows across all major subsystems:
//! token economy, staking, credit system, subnet economy, agent wallets,
//! settlement, treasury, genesis config, payment channels, and the top-level
//! Blockchain struct.
//!
//! Run with:
//!   cargo test --manifest-path core/blockchain/Cargo.toml --test smoke_test 2>&1 | tail -30

mod channel_chain_tests;
mod economy_tests;
mod helpers;
mod settlement_treasury_tests;
mod subnets_wallet_tests;
