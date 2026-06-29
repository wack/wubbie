//! The `wubbie tokenizer` command handler: train the byte-level BPE tokenizer.

use anyhow::Result;

use crate::config::TokenizerSubcommand;
use crate::{corpus, tokenizer};

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
        // Resolve the source (downloading from Hugging Face on demand) to local
        // shard paths.
        let source = self.args.corpus_source();
        let files = source.resolve_files()?;
        tracing::info!(
            files = files.len(),
            vocab_size = self.args.vocab_size(),
            min_frequency = self.args.min_frequency(),
            text_field = self.args.text_field(),
            "training byte-level BPE tokenizer",
        );

        let text_field = self.args.text_field().to_owned();
        let sequences = corpus::text_records(files.clone(), text_field.clone());
        let tokenizer = tokenizer::train_from_sequences(
            sequences,
            self.args.vocab_size(),
            self.args.min_frequency(),
        )?;

        tokenizer::save(&tokenizer, self.args.output())?;
        tracing::info!(output = %self.args.output().display(), "wrote tokenizer");

        // Verify the definition-of-done against a sample of the corpus. This
        // fails loudly if the round-trip or atomic-special-token guarantees are
        // violated, so a broken tokenizer never passes silently.
        let sample = corpus::read_sample(&files, &text_field, SAMPLE_CHARS)?;
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
