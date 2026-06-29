pub use colors::EnableColors;
pub use cors::CorsConfig;
pub use pg::PostgresConfig;

use clap::{Parser, Subcommand};

mod colors;
mod cors;
pub mod export_openapi;
mod pg;
pub mod server;
pub mod version;

#[derive(Debug, Clone, clap::ValueEnum, Default)]
pub enum LogFormat {
    #[default]
    Text,
    JSON,
}

#[derive(Debug, Clone, clap::Args)]
pub struct TlsConfig {
    /// Path to TLS certificate file
    #[arg(long = "tls-cert", env = "TLS_CERT", requires = "key")]
    pub cert: Option<String>,

    /// Path to TLS key file
    #[arg(long = "tls-key", env = "TLS_KEY", requires = "cert")]
    pub key: Option<String>,
}

#[derive(Debug, Parser, Clone)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Option<CliCommand>,
}

// `CliCommand` is parsed exactly once at process startup, so the size delta
// between the small `Version`/`ExportOpenapi` variants and the larger `Server`
// variant is irrelevant — boxing it would only add churn to the dispatch match.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand, Clone)]
pub enum CliCommand {
    /// Print the CLI version and exit
    Version(version::Version),
    /// Run the web server
    Server(server::Server),
    /// Export OpenAPI specification to stdout
    ExportOpenapi(export_openapi::ExportOpenapi),
}

impl CliCommand {
    pub async fn dispatch(self) -> miette::Result<()> {
        match self {
            CliCommand::Version(version) => version.dispatch(),
            CliCommand::Server(server) => server.dispatch().await,
            CliCommand::ExportOpenapi(export_openapi) => export_openapi.dispatch(),
        }
    }
}
