//! Test application wiring for BDD / integration tests.
//!
//! Builds a Salvo `Service` from the real controllers but with **in-memory mock
//! repositories** injected — BDD tests never touch a real database.

#![allow(dead_code)]

use std::sync::Arc;

use salvo::prelude::*;

use __service_name__::controllers::items::items_controller;
use __service_name__::repos::ItemStore;
use __service_name__::repos::items::mocks::MockItemStore;

/// A test application wrapping a Salvo `Service`.
pub struct TestApp {
    service: Service,
}

impl TestApp {
    /// Wraps a router in a `Service`.
    #[must_use]
    pub fn new(router: Router) -> Self {
        Self {
            service: Service::new(router),
        }
    }

    /// The underlying Salvo service (pass to `TestClient::send`).
    #[must_use]
    pub fn service(&self) -> &Service {
        &self.service
    }
}

/// Middleware that injects a fresh in-memory `MockItemStore` as the
/// `Arc<dyn ItemStore>` the controllers expect — no database, no I/O.
#[handler]
async fn inject_mock_item_store(depot: &mut Depot) {
    let store: Arc<dyn ItemStore> = Arc::new(MockItemStore::new());
    depot.inject(store);
}

/// Builds a test app that serves the real items controller backed by the
/// in-memory mock store.
#[must_use]
pub fn create_test_app() -> TestApp {
    let router = Router::new()
        .hoop(inject_mock_item_store)
        .push(items_controller());
    TestApp::new(router)
}
