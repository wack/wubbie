//! Migration round-trip integration test.
//!
//! Applies every migration (`up`), asserts the schema is present, then rolls the
//! whole chain back (`down`) and asserts it is gone — proving each migration's
//! `up`/`down` are inverses.
//!
//! This test needs a throwaway Postgres. It **skips green** when
//! `MIGRATION_TEST_DATABASE_URL` is unset, so `cargo test -p migration` passes
//! with or without a database (CI/local stay green; the real round-trip is
//! opt-in).
//!
//! # Running the real round-trip
//!
//! ```bash
//! cargo make pg   # or any throwaway Postgres
//! MIGRATION_TEST_DATABASE_URL=postgres://postgres:postgres@localhost/migration_test \
//!   cargo test -p migration --test round_trip
//! ```

use std::env;

use migration::Migrator;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;

const ENV_VAR: &str = "MIGRATION_TEST_DATABASE_URL";

/// Returns whether the `public.items` table exists in the connected database.
async fn items_table_exists(conn: &DatabaseConnection) -> bool {
    let stmt = Statement::from_string(
        DbBackend::Postgres,
        "SELECT to_regclass('public.items') IS NOT NULL AS present",
    );
    let row = conn
        .query_one(stmt)
        .await
        .expect("query items table existence")
        .expect("existence query returns a row");
    row.try_get::<bool>("", "present")
        .expect("read `present` column as bool")
}

#[tokio::test]
async fn migrations_apply_and_roll_back() {
    let url = match env::var(ENV_VAR) {
        Ok(url) if !url.is_empty() => url,
        _ => {
            eprintln!(
                "skipping migration round-trip test: set {ENV_VAR} \
                 (e.g. postgres://postgres:postgres@localhost/migration_test) to run"
            );
            return;
        }
    };

    // Safety: the schema reset below is destructive (`DROP SCHEMA ... CASCADE`).
    // Refuse to run unless the target database name looks like a throwaway test
    // DB, so a typo or a reused production URL cannot wipe real data.
    let db_name = url
        .rsplit('/')
        .next()
        .and_then(|tail| tail.split(['?', '#']).next())
        .unwrap_or("");
    assert!(
        db_name.contains("test"),
        "refusing to reset schema: {ENV_VAR} database name {db_name:?} must contain \
         \"test\" (point it at a throwaway DB, e.g. .../migration_test)"
    );

    let conn = Database::connect(&url)
        .await
        .expect("connect to test database");

    // Start from a clean schema so the round-trip is deterministic.
    conn.execute_unprepared("DROP SCHEMA IF EXISTS public CASCADE; CREATE SCHEMA public;")
        .await
        .expect("reset public schema");

    // Apply every migration, then confirm the schema is present.
    Migrator::up(&conn, None).await.expect("apply migrations");
    assert!(
        items_table_exists(&conn).await,
        "items table should exist after `up`"
    );

    // Roll the whole chain back, then confirm the schema is gone.
    Migrator::down(&conn, None)
        .await
        .expect("roll back migrations");
    assert!(
        !items_table_exists(&conn).await,
        "items table should be gone after `down`"
    );
}
