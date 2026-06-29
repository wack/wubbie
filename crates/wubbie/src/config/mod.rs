//! Configuration of the CLI (from flags/env) and of the model/run (from config
//! files) — the single source of truth the model, loss, and training loop all
//! instantiate against.
//!
//! The clap layer lives in [`cli`] (the [`Cli`] entrypoint) and [`command`]
//! (the subcommand enum and its dispatch), with one argument struct per
//! subcommand. The domain config — [`ModelConfig`], the named [`ModelSize`]s,
//! [`TrainingConfig`], and the reproducible [`RunConfig`] bundle — is
//! `serde`-serializable and format-agnostic.

pub use cli::Cli;
pub use command::Command;
pub use generate::GenerateSubcommand;
pub use loader::{LayeredConfig, ModelConfigArgs, load_model_config};
pub use model::{CONTEXT_LENGTH, ModelConfig, ModelSize, PartialModelConfig};
pub use run::{RunConfig, TrainingConfig};
pub use serve::ServeSubcommand;
pub use train::TrainSubcommand;

mod cli;
mod command;
mod generate;
mod loader;
mod model;
mod run;
mod serve;
mod train;
