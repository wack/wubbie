use salvo::cors::{AllowHeaders, AllowMethods, Cors};
use salvo::http::Method;
use salvo::otel::Metrics;
use salvo::prelude::*;
use serde::Serialize;

use crate::cli::CorsConfig;
use crate::middleware::{
    metrics::metrics_middleware, otel_http::OtelHttpMiddleware, stores::item_store_middleware,
    transaction_middleware,
};

pub mod items;

#[derive(Serialize, salvo::oapi::ToSchema)]
struct HeartbeatResponse {
    ok: String,
}

#[endpoint(tags("Health"), summary = "Health check endpoint")]
async fn heartbeat() -> Json<HeartbeatResponse> {
    Json(HeartbeatResponse {
        ok: "true".to_string(),
    })
}

/// Creates CORS middleware from configuration.
/// Allows all standard HTTP methods used by REST APIs.
/// For local development, automatically includes localhost origins.
fn create_cors_middleware(config: &CorsConfig) -> impl salvo::Handler {
    let origins = config.get_allowed_origins();

    tracing::info!("Configuring CORS with allowed origins: {:?}", origins);

    if !config.is_safe() {
        tracing::warn!(
            "UNSAFE CORS CONFIGURATION: Wildcard '*' origin detected. \
             This should NEVER be used in production as it exposes the API to CSRF attacks."
        );
    }

    // Convert Vec<String> to Vec<&str> for Salvo API
    let origin_refs: Vec<&str> = origins.iter().map(|s| s.as_str()).collect();

    let mut cors = Cors::new()
        .allow_origin(origin_refs)
        .allow_methods(AllowMethods::list([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ]))
        .allow_headers(AllowHeaders::any())
        .max_age(config.max_age);

    if config.allow_credentials {
        tracing::info!("CORS: Allowing credentials (cookies, authorization headers)");
        cors = cors.allow_credentials(true);
    }

    cors.into_handler()
}

/// Creates a minimal router for OpenAPI schema generation only.
/// Does not include middleware that requires database connections or tracing.
pub fn schema_router() -> Router {
    Router::new()
        .push(Router::with_path("heartbeat").get(heartbeat))
        .push(items::items_controller())
}

pub fn root(cors_config: &CorsConfig) -> Router {
    let router = Router::new()
        .hoop(OtelHttpMiddleware::new())
        .hoop(create_cors_middleware(cors_config))
        .hoop(Metrics::new())
        .hoop(metrics_middleware())
        .hoop(transaction_middleware())
        .hoop(item_store_middleware())
        .push(Router::with_path("heartbeat").get(heartbeat))
        .push(items::items_controller());

    let mut doc = OpenApi::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    doc.info.description = Some("API service".to_string());
    doc.info.license = Some(salvo::oapi::License::new("MIT"));
    doc = doc.merge_router(&router);

    router
        .unshift(doc.into_router("/api-doc/openapi.json"))
        .unshift(Scalar::new("/api-doc/openapi.json").into_router("scalar"))
}
