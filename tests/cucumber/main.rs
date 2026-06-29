//! Cucumber BDD test runner.
//!
//! Entry point for the BDD suite. Runs every `.feature` file under `spec/`.
//!
//! # Running
//!
//! ```bash
//! cargo test --test cucumber     # or: cargo make integ
//! ```
//!
//! # Layout
//!
//! - `spec/` — Gherkin feature files, organized by domain
//! - `tests/cucumber/world.rs` — `TestWorld` state shared across steps
//! - `tests/cucumber/steps/` — step definitions by domain
//! - `tests/common/` — shared test helpers (`TestApp`, `TestClient`)

use clap::Parser;
use cucumber::{StatsWriter, World};

// Shared test utilities (also used by unit/integration tests).
#[path = "../common/mod.rs"]
mod common;

mod steps;
mod world;

use world::TestWorld;

#[tokio::main]
async fn main() -> Result<(), ()> {
    let args = Cli::parse();

    // Support IDE test-inspector calls (list/ignored) without running anything.
    if args.list || args.ignored {
        return Ok(());
    }

    // Run every feature under spec/, skipping scenarios tagged @skip.
    let results = TestWorld::cucumber()
        .fail_on_skipped()
        .filter_run("spec/", |_feature, _rule, scenario| {
            !scenario.tags.iter().any(|t| t == "skip")
        })
        .await;

    if results.execution_has_failed() {
        eprintln!("Cucumber tests have failed.");
        return Err(());
    }

    println!("Cucumber tests completed successfully.");
    Ok(())
}

/// Command-line arguments for the cucumber runner (supports IDE integration).
#[derive(Debug, Parser)]
#[command(name = "cucumber")]
#[command(about = "BDD test runner")]
pub struct Cli {
    /// List all available tests without running them.
    #[arg(long, default_value_t = false)]
    list: bool,

    /// Output format for test results.
    #[arg(long, default_value_t = String::new())]
    format: String,

    /// Run only ignored tests.
    #[arg(long, default_value_t = false)]
    ignored: bool,
}
