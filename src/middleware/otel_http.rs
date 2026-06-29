use std::time::Instant;

use opentelemetry::trace::{Span, SpanKind, Status, Tracer};
use opentelemetry::{KeyValue, global};
use salvo::prelude::*;

/// Custom OpenTelemetry HTTP middleware that produces spans conforming to the
/// [OTel HTTP Semantic Conventions](https://opentelemetry.io/docs/specs/semconv/http/http-spans/).
///
/// This replaces Salvo's built-in `Logger` middleware which produces non-compliant spans:
/// - Uses `SPAN_KIND_SERVER` (kind=2) instead of `SPAN_KIND_INTERNAL`
/// - Sets `http.response.status_code` as an integer attribute instead of event string
/// - Uses namespaced attribute keys (`http.request.method`, `url.full`, etc.)
pub struct OtelHttpMiddleware;

impl Default for OtelHttpMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl OtelHttpMiddleware {
    pub fn new() -> Self {
        Self
    }
}

/// Extract the peer IP address from Salvo's remote address.
///
/// Salvo formats remote addresses as `socket://IP:PORT`. This extracts just the IP portion
/// to conform to the `network.peer.address` semantic convention.
fn extract_peer_address(req: &Request) -> String {
    let s = req.remote_addr().to_string();
    // Format is typically "socket://IP:PORT"
    let without_prefix = s.strip_prefix("socket://").unwrap_or(&s);
    without_prefix
        .rsplit_once(':')
        .map(|(ip, _port): (&str, &str)| ip.to_string())
        .unwrap_or_else(|| without_prefix.to_string())
}

/// Map HTTP version to the OTel `network.protocol.version` format.
fn format_protocol_version(version: salvo::http::Version) -> &'static str {
    match version {
        salvo::http::Version::HTTP_09 => "0.9",
        salvo::http::Version::HTTP_10 => "1.0",
        salvo::http::Version::HTTP_11 => "1.1",
        salvo::http::Version::HTTP_2 => "2",
        salvo::http::Version::HTTP_3 => "3",
        _ => "unknown",
    }
}

#[async_trait]
impl Handler for OtelHttpMiddleware {
    async fn handle(
        &self,
        req: &mut Request,
        depot: &mut Depot,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) {
        let tracer = global::tracer(env!("CARGO_PKG_NAME"));

        let method = req.method().as_str().to_owned();
        let path = req.uri().path().to_owned();
        let full_url = req.uri().to_string();
        let peer_address = extract_peer_address(req);
        let protocol_version = format_protocol_version(req.version());

        // OTel convention: span name is "METHOD /route"
        let span_name = format!("{} {}", method, path);

        let mut span = tracer
            .span_builder(span_name)
            .with_kind(SpanKind::Server)
            .with_attributes(vec![
                KeyValue::new("http.request.method", method.clone()),
                KeyValue::new("url.full", full_url),
                KeyValue::new("http.route", path.clone()),
                KeyValue::new("network.peer.address", peer_address.clone()),
                KeyValue::new("network.protocol.version", protocol_version),
            ])
            .start(&tracer);

        tracing::info!(
            method = %method,
            path = %path,
            peer = %peer_address,
            "Request received"
        );

        let start = Instant::now();

        // Process the request through the rest of the middleware chain
        ctrl.call_next(req, depot, res).await;

        // Set response status code as an integer attribute (OTel convention)
        let status_code = res.status_code.unwrap_or(StatusCode::OK).as_u16();
        let latency_ms = start.elapsed().as_millis();

        span.set_attribute(KeyValue::new(
            "http.response.status_code",
            i64::from(status_code),
        ));

        // OTel convention: set span status to Error for 5xx responses
        if status_code >= 500 {
            span.set_status(Status::error(format!("HTTP {status_code}")));
        }

        tracing::info!(
            method = %method,
            path = %path,
            status = status_code,
            latency_ms = latency_ms,
            "Response sent"
        );

        span.end();
    }
}
