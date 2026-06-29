//! HTTP client for the API.
//!
//! This crate provides a type-safe HTTP client generated from the API's
//! OpenAPI specification using [progenitor](https://github.com/oxidecomputer/progenitor).
//!
//! # Example
//!
//! ```no_run
//! use __service_name___client::Client;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = Client::new("http://localhost:8080");
//!
//! // Use the generated methods to interact with the API
//! let response = client.heartbeat().send().await?;
//! # Ok(())
//! # }
//! ```

// Include the generated client code from the build script
include!(concat!(env!("OUT_DIR"), "/codegen.rs"));
