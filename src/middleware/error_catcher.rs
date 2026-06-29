//! JSON error catcher for framework-level error responses.
//!
//! Salvo's default `DefaultGoal` produces HTML (or JSON / XML / plain text,
//! depending on the `Accept` header) when an extractor fails or when no
//! handler writes a body for an error status. Our API clients expect a
//! consistent JSON envelope, so this catcher hoop intercepts error responses
//! and rewrites them as:
//!
//! ```json
//! { "error": "BadRequest", "message": "Invalid request parameters" }
//! ```
//!
//! To use it, register it on the `Service`:
//!
//! ```ignore
//! Service::new(router).catcher(Catcher::default().hoop(JsonErrorCatcher));
//! ```

use salvo::async_trait;
use salvo::http::{ResBody, header};
use salvo::prelude::*;
use serde::Serialize;

/// A `Catcher` hoop that serializes framework-level error responses as JSON.
///
/// Runs before Salvo's `DefaultGoal` so that any `ResBody::Error(StatusError)`
/// (produced, for example, by `PathParam` or `JsonBody` extractors) is
/// replaced with a flat JSON envelope and the content-type is set to
/// `application/json`.
pub struct JsonErrorCatcher;

#[derive(Serialize)]
struct JsonErrorEnvelope<'a> {
    error: &'a str,
    message: String,
}

#[async_trait]
impl Handler for JsonErrorCatcher {
    async fn handle(
        &self,
        _req: &mut Request,
        _depot: &mut Depot,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) {
        let Some(status) = res.status_code else {
            return;
        };
        if !(status.is_client_error() || status.is_server_error()) {
            return;
        }

        // Only rewrite when Salvo has produced an error body. Handlers that
        // write their own response bodies (e.g. `Json<Result<_, _>>` from our
        // view layer) are left alone.
        let ResBody::Error(status_error) = &res.body else {
            return;
        };

        let envelope = JsonErrorEnvelope {
            error: status_name_to_code(status),
            message: message_for(status, &status_error.brief),
        };

        let Ok(body) = serde_json::to_string(&envelope) else {
            return;
        };

        res.headers_mut().insert(
            header::CONTENT_TYPE,
            "application/json"
                .parse()
                .expect("content-type header value should parse"),
        );
        let _ = res.write_body(body);
        ctrl.skip_rest();
    }
}

/// Maps an HTTP status code to the PascalCase `error` tag used in our
/// JSON error envelope. Kept in sync with the variants produced by our
/// view-layer `*Failure` enums.
fn status_name_to_code(status: StatusCode) -> &'static str {
    match status {
        StatusCode::BAD_REQUEST => "BadRequest",
        StatusCode::UNAUTHORIZED => "Unauthorized",
        StatusCode::FORBIDDEN => "Forbidden",
        StatusCode::NOT_FOUND => "NotFound",
        StatusCode::CONFLICT => "Conflict",
        StatusCode::UNPROCESSABLE_ENTITY => "UnprocessableEntity",
        StatusCode::INTERNAL_SERVER_ERROR => "InternalServerError",
        _ => "Error",
    }
}

/// Customize the human-readable message that accompanies a framework-level
/// error. Salvo's extractors emit `brief = "parse http data failed."` for
/// any parse failure, which is too generic for API clients; we rewrite it
/// to `"Invalid request parameters"`. Edit this function to add more
/// mappings as needed.
fn message_for(status: StatusCode, brief: &str) -> String {
    match status {
        StatusCode::BAD_REQUEST if brief == "parse http data failed." => {
            "Invalid request parameters".to_string()
        }
        _ => brief.to_string(),
    }
}
