use std::time::Duration;

use clap::Parser;

/// `PostgresConfig` enables a connection to a Postgres database,
/// and is required under almost all circumstances.
#[derive(Debug, Parser, Clone)]
pub struct PostgresConfig {
    /// Postgres host (url or unix socket, localhost while running locally)
    #[arg(long, env = "POSTGRES_HOST", default_value = "localhost")]
    pub pg_host: String,
    /// Postgres  database port
    #[arg(long = "postgres-port", env = "POSTGRES_PORT", default_value_t = 5432)]
    pub pg_port: u16,
    /// Postgres database
    #[arg(long, env = "POSTGRES_DATABASE", default_value = "postgres")]
    pub database: String,
    /// Postgres username
    #[arg(long, env = "POSTGRES_USER", default_value = "")]
    pub username: String,
    /// Postgres password
    #[arg(long, env = "POSTGRES_PASSWORD", default_value = "")]
    pub password: String,

    // default_value_t=chrono::Duration::from_millis(2500))]
    // The maximum amount of time spent waiting to acquire a connection to the DB.
    // See
    // [this link](https://docs.rs/sqlx/latest/sqlx/pool/struct.PoolOptions.html#method.acquire_timeout)
    #[arg(env = "POSTGRES_ACQUIRE_TIMEOUT", value_parser=parse_duration)]
    pub acquire_timeout: Option<Duration>,
}

fn parse_duration(arg: &str) -> Result<std::time::Duration, std::num::ParseIntError> {
    let seconds = arg.parse()?;
    Ok(std::time::Duration::from_secs(seconds))
}

impl PostgresConfig {
    /// Compose a PostgreSQL connection string from the configuration
    pub fn connection_string(&self) -> String {
        if self.password.is_empty() {
            format!(
                "postgres://{}@{}:{}/{}",
                self.username, self.pg_host, self.pg_port, self.database
            )
        } else {
            format!(
                "postgres://{}:{}@{}:{}/{}",
                self.username, self.password, self.pg_host, self.pg_port, self.database
            )
        }
    }
}
