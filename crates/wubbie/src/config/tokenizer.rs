//! Arguments for `wubbie tokenizer`.

use std::path::{Path, PathBuf};

use clap::{ArgGroup, Args};

use crate::corpus::{CorpusSource, DEFAULT_TEXT_FIELD, HfSource};
use crate::tokenizer::{DEFAULT_MIN_FREQUENCY, VOCAB_SIZE};

/// `wubbie tokenizer`: train the byte-level BPE tokenizer on a corpus slice.
///
/// The corpus comes from exactly one source: a local `--input` path (a file, or
/// a directory walked recursively for corpus files) or a pinned Hugging Face dataset
/// (`--hf-repo` at `--hf-revision`). The filtered CommonPile slice lives on HF
/// (MULTI-1378) and is pulled on demand; the local path covers tiny local runs.
/// Shards are JSONL/`.gz` (text under `--text-field`) or plain text.
///
/// It trains a GPT-2-style byte-level BPE to `--vocab-size`, reserves the locked
/// special-token inventory, writes a `tokenizer.json` to `--output`, then runs
/// the definition-of-done checks (round-trip, atomic special tokens, compression
/// ratio) against a sample of the corpus.
#[derive(Debug, Args, Clone)]
#[command(group(
    ArgGroup::new("corpus_source").required(true).args(["input", "hf_repo"])
))]
pub struct TokenizerSubcommand {
    /// Local corpus: a single file, or a directory walked recursively for
    /// `.jsonl`/`.jsonl.gz`/`.txt` files.
    #[arg(long, value_name = "PATH")]
    input: Option<PathBuf>,

    /// Hugging Face dataset repo id to pull the corpus from (e.g.
    /// `owner/filtered-commonpile`).
    #[arg(long, value_name = "REPO_ID")]
    hf_repo: Option<String>,

    /// Revision (branch, tag, or commit sha) of `--hf-repo`, pinned for
    /// reproducibility.
    #[arg(long, value_name = "REV", default_value = "main")]
    hf_revision: String,

    /// Specific file(s) within `--hf-repo` to fetch (repeatable). When omitted,
    /// every corpus file in the repo at the pinned revision is used.
    #[arg(long = "hf-file", value_name = "FILE")]
    hf_files: Vec<String>,

    /// Override the Hugging Face cache directory to read shards from. Use the
    /// same value you passed to `wubbie download`. Defaults to the hf-hub
    /// location (`HF_HOME` / `~/.cache/huggingface`).
    #[arg(long, value_name = "DIR")]
    cache_dir: Option<PathBuf>,

    /// JSON field holding each record's document text (JSONL sources only).
    #[arg(long, value_name = "FIELD", default_value = DEFAULT_TEXT_FIELD)]
    text_field: String,

    /// Where to write the trained tokenizer.
    #[arg(long, value_name = "FILE", default_value = "tokenizer.json")]
    output: PathBuf,

    /// Target total vocabulary size. Defaults to the locked size; override only
    /// when experimenting before the size is committed.
    #[arg(long, default_value_t = VOCAB_SIZE)]
    vocab_size: usize,

    /// Minimum pair frequency for a merge to be learned.
    #[arg(long, default_value_t = DEFAULT_MIN_FREQUENCY)]
    min_frequency: u64,
}

impl TokenizerSubcommand {
    /// Resolve the selected corpus source. The clap `ArgGroup` guarantees
    /// exactly one of `--input` / `--hf-repo` is present.
    pub fn corpus_source(&self) -> CorpusSource {
        match (&self.input, &self.hf_repo) {
            (_, Some(repo)) => CorpusSource::Hf(HfSource {
                repo: repo.clone(),
                revision: self.hf_revision.clone(),
                files: self.hf_files.clone(),
                cache_dir: self.cache_dir.clone(),
            }),
            (Some(input), None) => CorpusSource::Local(input.clone()),
            (None, None) => unreachable!("clap requires one of --input / --hf-repo"),
        }
    }

    /// The JSON field holding document text in JSONL sources.
    pub fn text_field(&self) -> &str {
        &self.text_field
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
    fn a_corpus_source_is_required() {
        assert!(parse(&["tokenizer"]).is_err());
    }

    #[test]
    fn input_and_hf_repo_are_mutually_exclusive() {
        let err = parse(&["tokenizer", "--input", "c.txt", "--hf-repo", "o/r"])
            .expect_err("two sources conflict");
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn local_source_and_defaults() {
        let args = parse(&["tokenizer", "--input", "corpus.txt"]).expect("parses");
        assert!(
            matches!(args.corpus_source(), CorpusSource::Local(p) if p == Path::new("corpus.txt"))
        );
        assert_eq!(args.text_field(), DEFAULT_TEXT_FIELD);
        assert_eq!(args.output(), Path::new("tokenizer.json"));
        assert_eq!(args.vocab_size(), VOCAB_SIZE);
        assert_eq!(args.min_frequency(), DEFAULT_MIN_FREQUENCY);
    }

    #[test]
    fn hf_source_pins_repo_revision_and_files() {
        let args = parse(&[
            "tokenizer",
            "--hf-repo",
            "owner/filtered-commonpile",
            "--hf-revision",
            "abc123",
            "--hf-file",
            "data/train-0.jsonl.gz",
            "--hf-file",
            "data/train-1.jsonl.gz",
            "--text-field",
            "content",
        ])
        .expect("parses");
        match args.corpus_source() {
            CorpusSource::Hf(source) => {
                assert_eq!(source.repo, "owner/filtered-commonpile");
                assert_eq!(source.revision, "abc123");
                assert_eq!(
                    source.files,
                    ["data/train-0.jsonl.gz", "data/train-1.jsonl.gz"]
                );
            }
            other => panic!("expected an HF source, got {other:?}"),
        }
        assert_eq!(args.text_field(), "content");
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
