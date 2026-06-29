//! Arguments for `wubbie train`.

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Args;

use super::{ModelConfig, ModelConfigArgs, ModelSize, load_model_config};
use crate::tokenizer;

/// `wubbie train`: train the model from a corpus.
///
/// Scaffold arguments — the training loop is implemented in a later ticket.
/// `--size` picks the named architecture base; `--config` layers a (possibly
/// partial) config file on top of it; individual `--d-model`/`--num-layers`/…
/// flags are the highest-precedence overrides. See
/// [`resolve_model_config`](Self::resolve_model_config).
#[derive(Debug, Args, Clone)]
pub struct TrainSubcommand {
    /// Path to a config file layered over `--size` (`.toml` or `.json`).
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Named model size used as the base layer.
    #[arg(long, value_enum, default_value_t = ModelSize::Gpt2Small)]
    size: ModelSize,

    /// Trained `tokenizer.json`. When given, the model's `vocab_size` is read
    /// from it — the tokenizer is the source of truth for vocabulary size, so it
    /// overrides every other layer (the embedding table must match the tokenizer
    /// exactly).
    // TODO(MULTI-1383): make this required once the training loop lands. A model
    // whose `vocab_size` doesn't match its tokenizer is always a bug; it is only
    // `Option` now so the stubbed `train` command and the config tests can run
    // without a tokenizer artifact.
    #[arg(long, value_name = "FILE")]
    tokenizer: Option<PathBuf>,

    /// Per-field model-dimension overrides (highest precedence among the layered
    /// sources; still superseded by `--tokenizer` for `vocab_size`).
    #[command(flatten)]
    model: ModelConfigArgs,
}

impl TrainSubcommand {
    /// Path to the config file, if one was supplied.
    pub fn config(&self) -> Option<&Path> {
        self.config.as_deref()
    }

    /// The named model size used as the base layer.
    pub fn size(&self) -> ModelSize {
        self.size
    }

    /// Path to the trained tokenizer, if one was supplied.
    pub fn tokenizer(&self) -> Option<&Path> {
        self.tokenizer.as_deref()
    }

    /// Resolve the fully-specified [`ModelConfig`] by merging, in increasing
    /// precedence: the `--size` base, the `--config` file, the `WUBBIE_MODEL_`
    /// environment layer, and the per-field override flags.
    ///
    /// If `--tokenizer` is supplied, `vocab_size` is then overridden with the
    /// trained tokenizer's *actual* size: the tokenizer is the authority on
    /// vocabulary size (the embedding table and LM head must match it), so it
    /// supersedes the layered value regardless of where that came from. The
    /// [`VOCAB_SIZE`](crate::tokenizer::VOCAB_SIZE) default only applies until a
    /// tokenizer exists.
    pub fn resolve_model_config(&self) -> Result<ModelConfig> {
        let mut config =
            load_model_config(&self.size.config(), self.config.as_deref(), &self.model)?;
        if let Some(path) = self.tokenizer.as_deref() {
            let vocab_size = tokenizer::vocab_size_from_file(path)?;
            if vocab_size != config.vocab_size {
                tracing::warn!(
                    configured = config.vocab_size,
                    tokenizer = vocab_size,
                    "overriding configured vocab_size with the trained tokenizer's actual size",
                );
            }
            config.vocab_size = vocab_size;
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    /// `Args` structs can't be parsed standalone; flatten into a tiny `Parser`.
    #[derive(Debug, Parser)]
    struct Harness {
        #[command(flatten)]
        args: TrainSubcommand,
    }

    fn parse(argv: &[&str]) -> TrainSubcommand {
        Harness::try_parse_from(argv).expect("parses").args
    }

    /// Train and save a tiny tokenizer of the requested size to a temp path.
    fn write_tiny_tokenizer(vocab_size: usize, tag: &str) -> PathBuf {
        let corpus: Vec<String> = (0..200)
            .map(|_| "the quick brown fox jumps over the lazy dog. tokens and text 123.".to_owned())
            .collect();
        let tok = tokenizer::train_from_sequences(corpus.into_iter(), vocab_size, 2)
            .expect("train tokenizer");
        let path = std::env::temp_dir().join(format!(
            "wubbie-train-tok-{}-{tag}.json",
            std::process::id(),
        ));
        tokenizer::save(&tok, &path).expect("save tokenizer");
        path
    }

    #[test]
    fn without_tokenizer_vocab_is_the_default() {
        let args = parse(&["train", "--size", "debug-tiny"]);
        let config = args.resolve_model_config().expect("resolves");
        assert_eq!(config.vocab_size, tokenizer::VOCAB_SIZE);
    }

    #[test]
    fn tokenizer_is_the_source_of_truth_for_vocab_size() {
        let path = write_tiny_tokenizer(300, "src");
        let args = parse(&[
            "train",
            "--size",
            "debug-tiny",
            "--tokenizer",
            path.to_str().expect("utf-8 path"),
        ]);
        let config = args.resolve_model_config().expect("resolves");
        assert_eq!(config.vocab_size, 300);
        std::fs::remove_file(&path).expect("cleanup");
    }

    #[test]
    fn tokenizer_supersedes_even_an_explicit_vocab_flag() {
        let path = write_tiny_tokenizer(300, "wins");
        let args = parse(&[
            "train",
            "--size",
            "debug-tiny",
            "--vocab-size",
            "9999",
            "--tokenizer",
            path.to_str().expect("utf-8 path"),
        ]);
        let config = args.resolve_model_config().expect("resolves");
        assert_eq!(config.vocab_size, 300);
        std::fs::remove_file(&path).expect("cleanup");
    }
}
