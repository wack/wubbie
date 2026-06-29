use std::sync::OnceLock;
use std::time::Duration;

use sea_orm::entity::prelude::DatabaseConnection;
use sea_orm::{ConnectOptions, Database};

use crate::cli::PostgresConfig;

pub static SEAORM_POOL: OnceLock<DatabaseConnection> = OnceLock::new();

pub async fn init(config: &PostgresConfig) {
    // Get the PostgreSQL connection string from the configuration
    let database_url = config.connection_string();

    let mut opt = ConnectOptions::new(database_url);
    opt.max_connections(20) // Using the same DEFAULT_MAX_CONNECTIONS from pg.rs
        .min_connections(5)
        .connect_timeout(config.acquire_timeout.unwrap_or(Duration::from_secs(3)))
        .idle_timeout(Duration::from_secs(8))
        .sqlx_logging(false);

    let pool = Database::connect(opt)
        .await
        .expect("db connection should connect");
    SEAORM_POOL.set(pool).expect("seaorm pool should be set");
}

pub fn pool() -> &'static DatabaseConnection {
    SEAORM_POOL.get().expect("seaorm pool should set")
}
