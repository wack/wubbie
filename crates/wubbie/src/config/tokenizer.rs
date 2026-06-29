//! Arguments for `wubbie tokenizer`.

use std::path::{Path, PathBuf};

use clap::Args;

use crate::tokenizer::{DEFAULT_MIN_FREQUENCY, VOCAB_SIZE};

/// `wubbie tokenizer`: train the byte-level BPE tokenizer on a corpus slice.
///
/// Reads `--input` (a file, or a directory swept for `.txt` files), trains a
/// GPT-2-style byte-level BPE to `--vocab-size`, reserves the locked
/// special-token inventory, and writes a `tokenizer.json` to `--output`. After
/// training it runs the definition-of-done checks (round-trip, atomic special
/// tokens, compression ratio) against a sample of the corpus.
#[derive(Debug, Args, Clone)]
pub struct TokenizerSubcommand {
    /// Corpus to train on: a single text file, or a directory swept
    /// (non-recursively) for `.txt` files.
    #[arg(long, value_name = "PATH")]
    input: PathBuf,

    /// Where to write the trained tokenizer.
    #[arg(long, value_name = "FILE", default_value = "tokenizer.json")]
    output: PathBuf,

    /// Target total vocabulary size. Defaults to the locked [`VOCAB_SIZE`];
    /// override only when experimenting before the size is committed.
    #[arg(long, default_value_t = VOCAB_SIZE)]
    vocab_size: usize,

    /// Minimum pair frequency for a merge to be learned.
    #[arg(long, default_value_t = DEFAULT_MIN_FREQUENCY)]
    min_frequency: u64,
}

impl TokenizerSubcommand {
    /// The corpus input path (file or directory).
    pub fn input(&self) -> &Path {
        &self.input
    }

    /// The output path for the trained `tokenizer.json`.
    pub fn output(&self) -> &Path {
        &self.output
    }

    /// The target total vocabulary size.
    pub fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    /// The minimum pair frequency for a merge to be learned.
    pub fn min_frequency(&self) -> u64 {
        self.min_frequency
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
        args: TokenizerSubcommand,
    }

    fn parse(argv: &[&str]) -> Result<TokenizerSubcommand, clap::Error> {
        Harness::try_parse_from(argv).map(|h| h.args)
    }

    #[test]
    fn input_is_required() {
        assert!(parse(&["tokenizer"]).is_err());
    }

    #[test]
    fn defaults_track_the_locked_constants() {
        let args = parse(&["tokenizer", "--input", "corpus.txt"]).expect("parses");
        assert_eq!(args.input(), Path::new("corpus.txt"));
        assert_eq!(args.output(), Path::new("tokenizer.json"));
        assert_eq!(args.vocab_size(), VOCAB_SIZE);
        assert_eq!(args.min_frequency(), DEFAULT_MIN_FREQUENCY);
    }

    #[test]
    fn flags_override_the_defaults() {
        let args = parse(&[
            "tokenizer",
            "--input",
            "corpus/",
            "--output",
            "tok.json",
            "--vocab-size",
            "16000",
            "--min-frequency",
            "3",
        ])
        .expect("parses");
        assert_eq!(args.output(), Path::new("tok.json"));
        assert_eq!(args.vocab_size(), 16_000);
        assert_eq!(args.min_frequency(), 3);
    }
}
