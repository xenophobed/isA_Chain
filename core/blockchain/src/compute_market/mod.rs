//! Compute Marketplace State Management
//!
//! This module handles the on-chain state for the decentralized compute marketplace.
//! It manages provider registration, job lifecycle, escrow, and dispute resolution.

pub mod disputes;
pub mod jobs;
pub mod providers;
pub mod queries;
pub mod types;

#[cfg(test)]
mod tests;

pub use types::*;
