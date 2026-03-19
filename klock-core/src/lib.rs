//! # klock-core
//!
//! The deterministic coordination kernel for the Klock protocol.
//! Provides O(1) conflict detection, Wait-Die scheduling, and
//! intent-based lease management for multi-agent systems.

pub mod client;
pub mod conflict;
pub mod infrastructure;
#[path = "infrastructure_in_memory.rs"]
pub mod infrastructure_in_memory;
#[cfg(feature = "sqlite")]
#[path = "infrastructure_sqlite.rs"]
pub mod infrastructure_sqlite;
pub mod scheduler;
pub mod state;
pub mod types;

#[cfg(test)]
mod conflict_test;
#[cfg(test)]
#[path = "infrastructure_test.rs"]
mod infrastructure_test;
#[cfg(test)]
mod scheduler_test;
#[cfg(test)]
mod state_test;
