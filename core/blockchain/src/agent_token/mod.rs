//! Agent Token — Bonding-Curve Tokens for Individual Agents
//!
//! This module implements agent-specific tokens backed by bonding curves,
//! allowing users to invest in agent performance. Each agent can have exactly
//! one token, and pricing follows a configurable curve tied to token supply.

pub mod curve;
pub mod factory;
pub mod types;

#[cfg(test)]
mod tests;

pub use curve::*;
pub use factory::*;
pub use types::*;
