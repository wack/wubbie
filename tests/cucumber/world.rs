//! Shared state for cucumber BDD steps.
//!
//! `TestWorld` carries the most recent HTTP response across step definitions.
//! Keep it small — add domain state only as real features need it.

#![allow(dead_code)]

use cucumber::World;
use salvo::http::StatusCode;

/// State shared across cucumber steps within a scenario.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct TestWorld {
    /// Status code from the most recent response.
    pub last_status: Option<StatusCode>,
    /// Parsed JSON body from the most recent response (the `data` envelope is
    /// unwrapped when present).
    pub last_body: Option<serde_json::Value>,
}

impl TestWorld {
    /// Creates an empty `TestWorld`.
    fn new() -> Self {
        Self {
            last_status: None,
            last_body: None,
        }
    }

    /// Records a response: stores its status and (best-effort) parsed JSON body.
    pub fn set_response(&mut self, status: StatusCode, body: &[u8]) {
        self.last_status = Some(status);
        self.last_body = serde_json::from_slice::<serde_json::Value>(body)
            .ok()
            .map(|parsed| parsed.get("data").cloned().unwrap_or(parsed));
    }

    /// True if the last response was a 2xx success.
    pub fn last_response_was_success(&self) -> bool {
        self.last_status.is_some_and(|status| status.is_success())
    }

    /// True if the last response was a 4xx client error.
    pub fn last_response_was_client_error(&self) -> bool {
        self.last_status
            .is_some_and(|status| status.is_client_error())
    }
}

impl Default for TestWorld {
    fn default() -> Self {
        Self::new()
    }
}

// NOTE: plain `#[test]` fns do not run in this target (`harness = false` —
// `main.rs` owns the runner), so unit tests for helpers live in `src/` modules,
// not here. This file holds only the cucumber `World` and its helpers.
