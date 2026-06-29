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
pub use download::DownloadSubcommand;
pub use generate::GenerateSubcommand;
pub use loader::{
    EnvOverrides, LayeredConfig, MODEL_ENV_PREFIX, ModelConfigArgs, load_model_config,
    parse_env_overrides, read_model_env_overrides,
};
pub use model::{CONTEXT_LENGTH, ModelConfig, ModelSize, PartialModelConfig};
pub use run::{RunConfig, TrainingConfig};
pub use serve::ServeSubcommand;
pub use tokenizer::TokenizerSubcommand;
pub use train::TrainSubcommand;

mod cli;
mod command;
mod download;
mod generate;
mod loader;
mod model;
mod run;
mod serve;
mod tokenizer;
mod train;
