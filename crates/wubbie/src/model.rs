//! Model definition.
//!
//! The transformer architecture is implemented in a follow-up ticket. For now
//! this module re-exports [`ModelConfig`](crate::config::ModelConfig) so
//! downstream code has a stable path to the architecture description.

pub use crate::config::ModelConfig;
