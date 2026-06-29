//! Shared test helpers for BDD and integration tests.
//!
//! - [`test_app`] — builds a Salvo `Service` from the real controllers with
//!   in-memory mock repositories (never a real database).
//! - [`test_client`] — an ergonomic wrapper over Salvo's test client.

#![allow(dead_code)]

pub mod test_app;
pub mod test_client;
