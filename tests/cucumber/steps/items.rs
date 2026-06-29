//! Step definitions for the items features.
//!
//! These exercise the real items controller backed by an in-memory mock store
//! (see `tests/common/test_app.rs`) — no database, no I/O.

use cucumber::{then, when};

use crate::common::test_app::create_test_app;
use crate::common::test_client::TestClient;
use crate::world::TestWorld;

/// Lists items via `GET /items` against the mock-backed test app.
#[when("the client lists items")]
async fn list_items(world: &mut TestWorld) {
    let app = create_test_app();
    let client = TestClient::new();

    let response = client.get("/items").send(app.service()).await;
    world.set_response(response.status, &response.body);
}

/// Asserts the most recent response was a success.
#[then("the response should be successful")]
async fn response_should_be_successful(world: &mut TestWorld) {
    assert!(
        world.last_response_was_success(),
        "expected a 2xx response, got: {:?}",
        world.last_status
    );
}
