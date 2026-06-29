use crate::cli::{CorsConfig, EnableColors, LogFormat, PostgresConfig, TlsConfig};
use crate::middleware::JsonErrorCatcher;
use clap::Args;
use mvc_helpers::DeploymentEnvironment;
use salvo::catcher::Catcher;
use salvo::conn::rustls::{Keycert, RustlsConfig};
use salvo::prelude::*;
use salvo::server::ServerHandle;
use tokio::signal;
use tracing::{info, level_filters::LevelFilter};

#[derive(Debug, Clone, Args)]
pub struct Server {
    /// Server hostname
    #[arg(long = "host", env = "HOST", default_value = "127.0.0.1")]
    pub host: String,

    /// Server port
    #[arg(long = "port", env = "PORT", default_value = "8080")]
    pub port: u16,

    /// PostgreSQL database configuration
    #[command(flatten)]
    pub pg: PostgresConfig,

    /// TLS configuration
    #[command(flatten)]
    pub tls: TlsConfig,

    /// CORS configuration
    #[command(flatten)]
    pub cors: CorsConfig,

    #[arg(long, env = "LOG_LEVEL", default_value = "info")]
    pub log_level: LevelFilter,

    #[arg(long, env = "LOG_FORMAT", default_value = "text")]
    pub log_format: LogFormat,

    #[arg(long, default_value = "auto")]
    pub enable_colors: EnableColors,

    /// Deployment environment for OpenTelemetry resource attributes
    #[arg(long, env = "OTEL_DEPLOYMENT_ENVIRONMENT", default_value_t)]
    pub deployment_environment: DeploymentEnvironment,

    /// API key for authenticating with the OTLP collector (sent as X-API-KEY header)
    #[arg(long = "otel-exporter-api-key", env = "OTEL_EXPORTER_API_KEY")]
    pub otel_exporter_api_key: Option<String>,

    /// Service version for OpenTelemetry (e.g. Helm chart calver tag). Falls back to crate version.
    #[arg(long, env = "SERVICE_VERSION")]
    pub service_version: Option<String>,
}

impl Server {
    pub async fn dispatch(self) -> miette::Result<()> {
        // Initialize OpenTelemetry tracing and logging subscriber
        // The guard ensures spans are flushed on shutdown when dropped
        let _telemetry_guard =
            crate::utils::telemetry::init_telemetry(crate::utils::telemetry::TelemetryConfig {
                log_level: self.log_level,
                log_format: self.log_format.clone(),
                enable_colors: self.enable_colors,
                deployment_environment: self.deployment_environment,
                otel_exporter_api_key: self.otel_exporter_api_key,
                service_version: self.service_version,
            });

        info!("Initializing database connection");

        // Initialize database
        crate::utils::db::init(&self.pg).await;

        // Build the listen address from host and port
        let listen_addr = format!("{}:{}", self.host, self.port);

        // Create the service with controllers, CORS, and tracing middleware.
        // Attach a catcher that rewrites framework-level error responses
        // (e.g. `PathParam` / `JsonBody` extractor failures) as our JSON
        // error envelope.
        let service = Service::new(crate::controllers::root(&self.cors))
            .catcher(Catcher::default().hoop(JsonErrorCatcher));

        info!(address = %listen_addr, "Starting server");

        // Check if TLS is configured
        if let (Some(cert), Some(key)) = (self.tls.cert, self.tls.key) {
            info!(
                url = format!(
                    "https://{}/scalar",
                    listen_addr.replace("0.0.0.0", "127.0.0.1")
                ),
                "OpenAPI documentation available"
            );

            let config = RustlsConfig::new(Keycert::new().cert(cert).key(key));
            let acceptor = TcpListener::new(listen_addr.clone())
                .rustls(config)
                .bind()
                .await;
            let salvo_server = salvo::Server::new(acceptor);

            // Spawn signal handler for graceful shutdown
            tokio::spawn(shutdown_signal(salvo_server.handle()));

            // Start the server
            salvo_server.serve(service).await;
        } else {
            info!(
                url = format!(
                    "http://{}/scalar",
                    listen_addr.replace("0.0.0.0", "127.0.0.1")
                ),
                "OpenAPI documentation available"
            );

            let acceptor = TcpListener::new(listen_addr).bind().await;
            let salvo_server = salvo::Server::new(acceptor);

            // Spawn signal handler for graceful shutdown
            tokio::spawn(shutdown_signal(salvo_server.handle()));

            // Start the server
            salvo_server.serve(service).await;
        }

        // When we reach here, the server has stopped.
        // The _telemetry_guard will be dropped, flushing any pending spans.
        info!("Server shutdown complete, flushing telemetry...");

        Ok(())
    }
}

async fn shutdown_signal(handle: ServerHandle) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Ctrl+C signal received, shutting down gracefully...");
        },
        _ = terminate => {
            info!("Terminate signal received, shutting down gracefully...");
        },
    }

    info!("Initiating graceful shutdown with 60 second timeout...");
    handle.stop_graceful(Some(std::time::Duration::from_secs(60)));
}
