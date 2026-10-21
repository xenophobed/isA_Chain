//! Agent Registry — On-Chain Agent Discovery and Management
//!
//! This module provides an on-chain registry for AI agents, allowing them to
//! register their capabilities and pricing, and enabling callers to discover
//! agents by capability.

pub mod registry;
pub mod types;

#[cfg(test)]
mod tests;

pub use registry::*;
pub use types::*;
