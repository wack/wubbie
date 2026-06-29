//! Step definitions for the monitoring features (heartbeat).

use cucumber::{given, then};
use salvo::prelude::*;

use __service_name__::controllers::schema_router;

use crate::common::test_client::TestClient;
use crate::world::TestWorld;

/// Sends a request to the real `heartbeat` route and records the response.
///
/// Uses the production `schema_router()` (the heartbeat route needs no database
/// middleware), so a regression in the real route fails this scenario.
#[given("a request is sent to the heartbeat route")]
async fn request_heartbeat(world: &mut TestWorld) {
    let service = Service::new(schema_router());
    let client = TestClient::new();

    let response = client.get("/heartbeat").send(&service).await;
    world.set_response(response.status, &response.body);
}

/// Asserts the most recent response was a success.
#[then("the request should succeed")]
async fn request_should_succeed(world: &mut TestWorld) {
    assert!(
        world.last_response_was_success(),
        "expected a 2xx response, got: {:?}",
        world.last_status
    );
}
