use clap::Args;

#[derive(Debug, Clone, Args)]
pub struct DbConfig {
    /// Database URL for the primary database
    #[arg(
        long = "db-url",
        env = "DATABASE_URL",
        help = "Settings for the primary database. This is usually writeable, but will be read-only in some configurations."
    )]
    pub url: String,

    /// Minimum number of database connections
    #[arg(
        long = "db-min-connections",
        env = "DB_MIN_CONNECTIONS",
        default_value = "5"
    )]
    pub min_connections: u32,

    /// Maximum number of database connections
    #[arg(
        long = "db-max-connections",
        env = "DB_MAX_CONNECTIONS",
        default_value = "100"
    )]
    pub max_connections: u32,

    /// Database connection idle timeout in seconds
    #[arg(
        long = "db-idle-timeout",
        env = "DB_IDLE_TIMEOUT",
        default_value = "8"
    )]
    pub idle_timeout: u32,

    /// Database connection timeout in seconds
    #[arg(
        long = "db-connect-timeout",
        env = "DB_CONNECT_TIMEOUT",
        default_value = "8"
    )]
    pub connect_timeout: u32,

    /// Enable SQLx logging
    #[arg(
        long = "db-sqlx-logging",
        env = "DB_SQLX_LOGGING",
        default_value = "false"
    )]
    pub sqlx_logging: bool,
}
