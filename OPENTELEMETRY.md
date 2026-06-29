# OpenTelemetry Integration

This project includes OpenTelemetry tracing and custom HTTP metrics collection.

## Features

### 1. Distributed Tracing

OpenTelemetry tracing is automatically initialized when the server starts. The tracer is configured to:
- Export traces via OTLP HTTP protocol
- Use trace context propagation for distributed tracing
- Include all HTTP requests in traces via the Salvo `Tracing` middleware

The tracer is available in request handlers through the depot:
```rust
#[handler]
async fn my_handler(depot: &mut Depot) -> String {
    let tracer = depot.obtain::<Arc<Tracer>>().unwrap();
    // Use tracer for custom spans
}
```

### 2. HTTP Status Code Metrics

A custom metrics middleware collects HTTP status codes and emits them as histograms every minute.

**Implementation Details:**
- **RwLock-based synchronization**: Uses a single `HashMap` wrapped in `Arc<RwLock<>>` for thread-safe access
- **Concurrent reads**: Multiple requests can read concurrently; writes acquire exclusive lock briefly
- **Delta temporality**: Emits count deltas every 60 seconds, then resets the histogram
- **Automatic background task**: Spawns a tokio task that runs for the lifetime of the application

**Metrics Output:**
Every minute, the middleware logs:
```
INFO HTTP Status Code Metrics (delta): {200: 1523, 404: 42, 500: 3}
INFO status_code=200 count=1523 "HTTP status code count"
INFO status_code=404 count=42 "HTTP status code count"
INFO status_code=500 count=3 "HTTP status code count"
```

## Configuration

### OTLP Endpoint

By default, the OTLP exporter sends traces to `http://localhost:4318` (HTTP). To customize:

Set the `OTEL_EXPORTER_OTLP_ENDPOINT` environment variable:
```bash
export OTEL_EXPORTER_OTLP_ENDPOINT=http://my-collector:4318
```

### Service Name

The service name is automatically set from the Cargo package name. You can customize it by modifying `src/utils/telemetry.rs`.

## Middleware Order

The middleware is applied in this order (from outermost to innermost):
1. `Logger` - Request logging
2. `affix_state::inject` - Inject tracer into depot
3. `Metrics` - Salvo built-in metrics
4. `Tracing` - OpenTelemetry tracing
5. `metrics_middleware` - Custom HTTP status code collection
6. `transaction_middleware` - Database transaction management
7. `api_key_store_middleware` - API key store injection

This ensures that all requests are traced and metrics are collected before database transactions begin.

## Dependencies

The following OpenTelemetry dependencies are included:
- `opentelemetry` (0.31)
- `opentelemetry_sdk` (0.31) with features: `experimental_async_runtime`, `rt-tokio`
- `opentelemetry-otlp` (0.31) with features: `tonic`, `default`
- `opentelemetry-http` (0.31)

## Files

- `src/utils/telemetry.rs` - OpenTelemetry initialization
- `src/middleware/metrics.rs` - HTTP status code metrics middleware
- `src/controllers/mod.rs` - Router setup with tracing middleware
- `src/bin/main.rs` - Tracer initialization in main function
