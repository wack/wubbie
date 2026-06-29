//! The `wubbie tokenizer` command handler: train the byte-level BPE tokenizer.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

use crate::config::TokenizerSubcommand;
use crate::tokenizer;

/// How many characters of the corpus to sample for the post-training
/// definition-of-done checks (round-trip, compression ratio). Enough to be
/// representative without re-reading the whole slice.
const SAMPLE_CHARS: usize = 64 * 1024;

/// Handler for `wubbie tokenizer`.
pub struct Tokenizer {
    args: TokenizerSubcommand,
}

impl Tokenizer {
    pub fn new(args: TokenizerSubcommand) -> Result<Self> {
        Ok(Self { args })
    }

    pub fn dispatch(self) -> Result<()> {
        let files = collect_corpus_files(self.args.input())?;
        tracing::info!(
            files = files.len(),
            vocab_size = self.args.vocab_size(),
            min_frequency = self.args.min_frequency(),
            "training byte-level BPE tokenizer",
        );

        let tokenizer =
            tokenizer::train_from_files(&files, self.args.vocab_size(), self.args.min_frequency())?;

        tokenizer::save(&tokenizer, self.args.output())?;
        tracing::info!(output = %self.args.output().display(), "wrote tokenizer");

        // Verify the definition-of-done against a sample of the corpus. This
        // fails loudly if the round-trip or atomic-special-token guarantees are
        // violated, so a broken tokenizer never passes silently.
        let sample = read_sample(&files, SAMPLE_CHARS)?;
        let report = tokenizer::report(&tokenizer, &sample)?;
        tracing::info!(
            vocab_size = report.vocab_size,
            chars_per_token = report.chars_per_token,
            round_trips = report.round_trips,
            "tokenizer acceptance checks passed",
        );

        // The compression target is advisory (the corpus and vocab determine
        // it), so a miss is a warning, not a failure.
        if !(3.0..=4.5).contains(&report.chars_per_token) {
            tracing::warn!(
                chars_per_token = report.chars_per_token,
                "compression ratio is outside the expected ~3.5–4 band; \
                 inspect the corpus and vocab size",
            );
        }

        println!(
            "Trained tokenizer: vocab_size={}, chars/token={:.2} (sample), round-trip OK → {}",
            report.vocab_size,
            report.chars_per_token,
            self.args.output().display(),
        );
        Ok(())
    }
}

/// Resolve the `--input` path to the list of corpus files to train on.
///
/// A file is used directly; a directory is swept (non-recursively) for `.txt`
/// files, sorted for deterministic ordering.
fn collect_corpus_files(input: &Path) -> Result<Vec<PathBuf>> {
    let metadata = fs::metadata(input)
        .with_context(|| format!("cannot read corpus input: {}", input.display()))?;

    if metadata.is_file() {
        return Ok(vec![input.to_path_buf()]);
    }

    let mut files: Vec<PathBuf> = fs::read_dir(input)
        .with_context(|| format!("cannot read corpus directory: {}", input.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    files.sort();

    ensure!(
        !files.is_empty(),
        "no `.txt` files found in corpus directory: {}",
        input.display(),
    );
    Ok(files)
}

/// Read up to `max_chars` characters from the corpus for the sanity checks.
///
/// Reads whole lines so a multi-byte character is never split, stopping once the
/// budget is reached.
fn read_sample(files: &[PathBuf], max_chars: usize) -> Result<String> {
    let mut sample = String::new();
    let mut chars = 0;
    'outer: for path in files {
        let file = fs::File::open(path)
            .with_context(|| format!("cannot open corpus file: {}", path.display()))?;
        for line in BufReader::new(file).lines() {
            let line = line.with_context(|| format!("error reading {}", path.display()))?;
            chars += line.chars().count() + 1; // +1 for the '\n' we re-add
            sample.push_str(&line);
            sample.push('\n');
            if chars >= max_chars {
                break 'outer;
            }
        }
    }
    if sample.is_empty() {
        bail!("corpus is empty: nothing to sample for the acceptance checks");
    }
    Ok(sample)
}
