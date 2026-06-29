//! The top-level `wubbie` CLI definition (clap).

use clap::Parser;
use derive_getters::Getters;
use tracing::level_filters::LevelFilter;

use super::command::Command;

/// wubbie — a fully-open SLM, from corpus to inference.
#[derive(Debug, Getters, Parser)]
#[command(name = "wubbie", version, about, long_about = None)]
pub struct Cli {
    /// The subcommand to execute.
    #[command(subcommand)]
    cmd: Option<Command>,

    /// Sets the maximum log level. Defaults to INFO. Options are 'trace',
    /// 'debug', 'info', 'warn', 'error', and 'off' ('off' disables logging).
    /// Options are case-insensitive.
    #[arg(long, env, global = true, default_value_t = LevelFilter::INFO)]
    log_level: LevelFilter,
}
