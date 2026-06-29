use mvc_helpers::DeploymentEnvironment;
use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::cli::{EnableColors, LogFormat};

/// Service name for OpenTelemetry semantic conventions
const SERVICE_NAME: &str = env!("CARGO_PKG_NAME");

/// Service version from Cargo.toml
const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Header name for authenticating with the OTLP collector
const API_KEY_HEADER: &str = "X-API-KEY";

// Build-time provenance attributes, populated by `build.rs` from CI env vars
// (preferred) or local git shell-out. Missing values are stamped "unknown".
// Names follow the OpenTelemetry CI/CD + VCS semantic conventions.
const VCS_REF_HEAD_REVISION: &str = env!("VCS_REF_HEAD_REVISION");
const VCS_REF_HEAD_NAME: &str = env!("VCS_REF_HEAD_NAME");
const VCS_REF_HEAD_TYPE: &str = env!("VCS_REF_HEAD_TYPE");
const VCS_REPOSITORY_URL_FULL: &str = env!("VCS_REPOSITORY_URL_FULL");
const VCS_PROVIDER_NAME: &str = "github";
const CICD_PIPELINE_NAME: &str = env!("CICD_PIPELINE_NAME");
const CICD_PIPELINE_RUN_URL_FULL: &str = env!("CICD_PIPELINE_RUN_URL_FULL");

/// Guard that ensures OpenTelemetry resources are properly flushed on shutdown.
/// When dropped, this will flush any pending spans to the collector.
pub struct TelemetryGuard {
    provider: SdkTracerProvider,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Err(e) = self.provider.shutdown() {
            eprintln!("Failed to shutdown tracer provider: {e}");
        }
    }
}

/// Configuration for initializing telemetry.
pub struct TelemetryConfig {
    pub log_level: LevelFilter,
    pub log_format: LogFormat,
    pub enable_colors: EnableColors,
    pub deployment_environment: DeploymentEnvironment,
    pub otel_exporter_api_key: Option<String>,
    pub service_version: Option<String>,
}

/// Initialize the OpenTelemetry tracer provider and tracing subscriber.
///
/// This sets up:
/// - A tracing-subscriber with either text (colored) or JSON formatting
/// - A global OTel TracerProvider for OTel-compliant span export via OTLP
/// - W3C Trace Context propagation for distributed tracing
/// - OTel semantic convention resource attributes (service.name, service.version, deployment.environment.name)
///
/// Returns a TelemetryGuard. The guard must be held for the lifetime of the
/// application to ensure spans are flushed on shutdown.
pub fn init_telemetry(config: TelemetryConfig) -> TelemetryGuard {
    // Set up W3C Trace Context propagation for distributed tracing
    global::set_text_map_propagator(TraceContextPropagator::new());

    // Use runtime service version if provided, otherwise fall back to compile-time crate version
    let version = config.service_version.as_deref().unwrap_or(SERVICE_VERSION);

    // Build resource with OTel semantic convention attributes
    let attributes = vec![
        KeyValue::new("service.name", SERVICE_NAME),
        KeyValue::new("service.version", version.to_owned()),
        KeyValue::new(
            "deployment.environment.name",
            config.deployment_environment.to_string(),
        ),
        KeyValue::new("vcs.ref.head.revision", VCS_REF_HEAD_REVISION),
        KeyValue::new("vcs.ref.head.name", VCS_REF_HEAD_NAME),
        KeyValue::new("vcs.ref.head.type", VCS_REF_HEAD_TYPE),
        KeyValue::new("vcs.repository.url.full", VCS_REPOSITORY_URL_FULL),
        KeyValue::new("vcs.provider.name", VCS_PROVIDER_NAME),
        KeyValue::new("cicd.pipeline.name", CICD_PIPELINE_NAME),
        KeyValue::new("cicd.pipeline.run.url.full", CICD_PIPELINE_RUN_URL_FULL),
    ];
    let resource = Resource::builder().with_attributes(attributes).build();

    // Build custom headers for OTLP exporter
    let mut headers = std::collections::HashMap::new();

    // Attach API key via X-API-KEY header if configured
    if let Some(api_key) = config.otel_exporter_api_key {
        headers.insert(API_KEY_HEADER.to_string(), api_key);
    }

    // Create the OTLP exporter for sending spans to a collector (e.g., Vector)
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_headers(headers)
        .build()
        .expect("failed to create OTLP exporter");

    // Build the tracer provider with batch export and resource attributes
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    // Register the provider globally so it can be accessed from anywhere
    // The OtelHttpMiddleware uses global::tracer() to create OTel-compliant spans
    global::set_tracer_provider(provider.clone());

    // Configure and initialize the appropriate formatting layer
    match config.log_format {
        LogFormat::Text => init_text_subscriber(config.log_level, config.enable_colors),
        LogFormat::JSON => init_json_subscriber(config.log_level),
    }

    TelemetryGuard { provider }
}

/// Initialize the tracing subscriber with a human-readable text formatter.
fn init_text_subscriber(log_level: LevelFilter, enable_colors: EnableColors) {
    let ansi = match enable_colors {
        EnableColors::Always => true,
        EnableColors::Never => false,
        EnableColors::Auto => std::io::IsTerminal::is_terminal(&std::io::stderr()),
    };

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(ansi)
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false);

    tracing_subscriber::registry()
        .with(log_level)
        .with(fmt_layer)
        .init();
}

/// Initialize the tracing subscriber with a JSON formatter.
fn init_json_subscriber(log_level: LevelFilter) {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_span_events(FmtSpan::CLOSE)
        .with_current_span(true)
        .with_span_list(false)
        .flatten_event(true);

    tracing_subscriber::registry()
        .with(log_level)
        .with(fmt_layer)
        .init();
}
