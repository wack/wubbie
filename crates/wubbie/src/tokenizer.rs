//! Byte-level BPE tokenizer: the locked special-token inventory, training,
//! loading, and the definition-of-done sanity checks (MULTI-1379).
//!
//! wubbie uses the HuggingFace [`tokenizers`] crate. The tokenizer is a
//! GPT-2-style **byte-level BPE**: a byte-level pre-tokenizer maps every one of
//! the 256 bytes into a printable-character alphabet, so the model never needs
//! an `unk` token and `decode(encode(text)) == text` holds for arbitrary UTF-8.
//!
//! The vocabulary size and the special-token inventory are **locked here** —
//! both feed downstream phases (the Phase 2 model config sizes its embedding
//! table to [`VOCAB_SIZE`]; the Phase 3 chat template and Phase 6 serving path
//! render with exactly [`SPECIAL_TOKENS`]) and cannot be changed afterwards
//! without retraining.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use tokenizers::models::TrainerWrapper;
use tokenizers::models::bpe::{BPE, BpeTrainer};
use tokenizers::pre_tokenizers::byte_level::ByteLevel;
use tokenizers::{AddedToken, Tokenizer};

/// Vocabulary size of the locked tokenizer.
///
/// Locked at 32k — the top of the ~16–32k target band (MULTI-1379). The Phase 2
/// model config reads its `vocab_size` from this single constant, and the
/// loss-at-init ≈ `ln(VOCAB_SIZE)` sanity check is taken against it, so this is
/// the one place the size is defined. The trainer targets this total (special
/// tokens + the 256-byte alphabet + learned merges); a corpus large enough to
/// support that many merges lands the trained vocabulary exactly here.
pub const VOCAB_SIZE: usize = 32_000;

/// Padding token. Listed first in [`SPECIAL_TOKENS`] so the trainer assigns it
/// id `0`, the conventional pad / ignore index.
pub const PAD_TOKEN: &str = "<|pad|>";
/// Beginning-of-sequence marker.
pub const BOS_TOKEN: &str = "<|bos|>";
/// End-of-sequence marker.
pub const EOS_TOKEN: &str = "<|eos|>";
/// Chat-template turn-start marker (ChatML-style `<|im_start|>`).
pub const TURN_START_TOKEN: &str = "<|im_start|>";
/// Chat-template turn-end marker (ChatML-style `<|im_end|>`).
pub const TURN_END_TOKEN: &str = "<|im_end|>";

/// The locked special-token inventory, in vocabulary-id order.
///
/// **Fixed here and cannot be extended later** (MULTI-1379): the model's
/// embedding table is sized to include these reserved ids, and the chat
/// template (Phase 3, SFT) and serving path (Phase 6) render with exactly these
/// tokens. Only the rendering *format* is finalized later — the tokens
/// themselves are reserved now, each as a single atomic token. The order is
/// load-bearing: the trainer assigns ids `0..SPECIAL_TOKENS.len()` in this
/// order, so `PAD_TOKEN` is pinned to id `0`.
pub const SPECIAL_TOKENS: [&str; 5] = [
    PAD_TOKEN,
    BOS_TOKEN,
    EOS_TOKEN,
    TURN_START_TOKEN,
    TURN_END_TOKEN,
];

/// Default minimum pair frequency for a merge to be learned.
///
/// `2` drops hapax pairs (anything seen once) from the merge table — they can't
/// generalize — while keeping every pair with real repetition. The CLI exposes
/// this as `--min-frequency`.
pub const DEFAULT_MIN_FREQUENCY: u64 = 2;

/// Build an untrained byte-level BPE tokenizer, wired up GPT-2-style with a
/// byte-level pre-tokenizer, decoder, and post-processor.
///
/// `add_prefix_space` is deliberately **off**: with it on, the pre-tokenizer
/// injects a leading space into every input, which the byte-level decoder then
/// reproduces verbatim — breaking the exact `decode(encode(text)) == text`
/// round-trip the DoD requires. The post-processor only trims offsets; it adds
/// no special tokens, so encoding never silently injects BOS/EOS.
fn build_byte_level_bpe() -> Tokenizer {
    // The byte-level alphabet covers all 256 bytes, so `BPE::default()` (no
    // `unk` token) suffices — nothing ever falls outside the alphabet.
    let mut tokenizer = Tokenizer::new(BPE::default());
    // `ByteLevel` is `Copy`, so the same configured value serves as
    // pre-tokenizer, decoder, and post-processor.
    let byte_level = ByteLevel::new(
        false, // add_prefix_space — off, see above
        true,  // trim_offsets
        true,  // use_regex — the standard GPT-2 splitting pattern
    );
    tokenizer
        .with_pre_tokenizer(Some(byte_level))
        .with_decoder(Some(byte_level))
        .with_post_processor(Some(byte_level));
    tokenizer
}

/// Build the BPE trainer for the locked configuration.
///
/// Seeds the full 256-byte alphabet so every byte is representable from the
/// start, and reserves [`SPECIAL_TOKENS`] as atomic, non-normalized tokens at
/// the front of the vocabulary.
fn build_trainer(vocab_size: usize, min_frequency: u64, show_progress: bool) -> TrainerWrapper {
    let special_tokens = SPECIAL_TOKENS
        .iter()
        .map(|tok| AddedToken::from(*tok, true))
        .collect();
    BpeTrainer::builder()
        .vocab_size(vocab_size)
        .min_frequency(min_frequency)
        .special_tokens(special_tokens)
        // `ByteLevel::alphabet()` is an `ahash` set; the builder wants a
        // `std::collections::HashSet`.
        .initial_alphabet(ByteLevel::alphabet().into_iter().collect::<HashSet<_>>())
        .show_progress(show_progress)
        .build()
        .into()
}

/// Train a byte-level BPE tokenizer on a set of corpus files.
///
/// Each file is read line by line. `vocab_size` is the target *total*
/// vocabulary (special tokens + byte alphabet + learned merges); `min_frequency`
/// is the floor on pair frequency for a merge to be kept. Returns the trained,
/// ready-to-use [`Tokenizer`].
pub fn train_from_files(
    files: &[impl AsRef<Path>],
    vocab_size: usize,
    min_frequency: u64,
) -> Result<Tokenizer> {
    ensure!(!files.is_empty(), "no corpus files provided to train on");
    ensure!(
        vocab_size > SPECIAL_TOKENS.len() + 256,
        "vocab_size {vocab_size} is too small: it must exceed the {} special tokens plus the \
         256-byte alphabet",
        SPECIAL_TOKENS.len(),
    );

    let paths = files
        .iter()
        .map(|path| {
            let path = path.as_ref();
            path.to_str()
                .map(str::to_owned)
                .with_context(|| format!("corpus path is not valid UTF-8: {}", path.display()))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut tokenizer = build_byte_level_bpe();
    let mut trainer = build_trainer(vocab_size, min_frequency, true);
    tokenizer
        .train_from_files(&mut trainer, paths)
        .map_err(|err| anyhow::anyhow!("failed to train tokenizer: {err}"))?;
    Ok(tokenizer)
}

/// Save a trained tokenizer to a `tokenizer.json` file (pretty-printed).
pub fn save(tokenizer: &Tokenizer, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    tokenizer
        .save(path, true)
        .map_err(|err| anyhow::anyhow!("failed to save tokenizer to {}: {err}", path.display()))
}

/// Load a serialized tokenizer from a `tokenizer.json` file.
pub fn load(path: impl AsRef<Path>) -> Result<Tokenizer> {
    Tokenizer::from_file(path.as_ref())
        .map_err(|err| anyhow::anyhow!("failed to load tokenizer: {err}"))
}

/// The acceptance-criteria measurements for a trained tokenizer (MULTI-1379).
///
/// Produced by [`report`] and surfaced by `wubbie tokenizer` so the
/// definition-of-done is checked against the freshly trained artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Trained vocabulary size, including added special tokens.
    pub vocab_size: usize,
    /// Average characters per token over the sample text — the compression
    /// sanity check (target ~3.5–4; near 1 means merges aren't forming).
    pub chars_per_token: f64,
    /// Whether `decode(encode(sample)) == sample` held on the sample.
    pub round_trips: bool,
}

/// Encode `text` and assert it decodes back byte-for-byte.
///
/// Special tokens are *not* skipped on decode, but the sample text contains
/// none, so this is the pure byte-level round-trip the DoD calls for.
pub fn round_trips(tokenizer: &Tokenizer, text: &str) -> Result<bool> {
    let encoding = tokenizer
        .encode(text, false)
        .map_err(|err| anyhow::anyhow!("failed to encode sample: {err}"))?;
    let decoded = tokenizer
        .decode(encoding.get_ids(), false)
        .map_err(|err| anyhow::anyhow!("failed to decode sample: {err}"))?;
    Ok(decoded == text)
}

/// Average characters per token when encoding `text`.
///
/// Returns `0.0` for empty input (no tokens). Counts Unicode scalar values, not
/// bytes, so the ratio reads in human terms.
pub fn chars_per_token(tokenizer: &Tokenizer, text: &str) -> Result<f64> {
    let encoding = tokenizer
        .encode(text, false)
        .map_err(|err| anyhow::anyhow!("failed to encode sample: {err}"))?;
    let tokens = encoding.get_ids().len();
    if tokens == 0 {
        return Ok(0.0);
    }
    Ok(text.chars().count() as f64 / tokens as f64)
}

/// Verify every reserved special token encodes to exactly one atomic token.
///
/// This is the DoD guarantee that BOS/EOS, the turn markers, and pad survive as
/// single ids rather than being split into byte pieces. Returns an error naming
/// the first token that fails.
pub fn verify_special_tokens_atomic(tokenizer: &Tokenizer) -> Result<()> {
    for token in SPECIAL_TOKENS {
        let encoding = tokenizer
            .encode(token, false)
            .map_err(|err| anyhow::anyhow!("failed to encode special token {token:?}: {err}"))?;
        let ids = encoding.get_ids();
        ensure!(
            ids.len() == 1,
            "special token {token:?} did not encode atomically: got {} tokens {ids:?}",
            ids.len(),
        );
        ensure!(
            tokenizer.token_to_id(token).is_some(),
            "special token {token:?} is not present in the vocabulary",
        );
    }
    Ok(())
}

/// Compute the [`Report`] for `tokenizer` against `sample` text, and fail if any
/// acceptance criterion is violated.
///
/// Enforces, as hard errors: the byte-level round-trip and atomic special
/// tokens. The numeric measurements (vocab size, chars/token) are returned for
/// the caller to log and judge against the ~3.5–4 compression target.
pub fn report(tokenizer: &Tokenizer, sample: &str) -> Result<Report> {
    verify_special_tokens_atomic(tokenizer)?;
    let round_trips = round_trips(tokenizer, sample)?;
    ensure!(
        round_trips,
        "round-trip failed: decode(encode(sample)) != sample",
    );
    Ok(Report {
        vocab_size: tokenizer.get_vocab_size(true),
        chars_per_token: chars_per_token(tokenizer, sample)?,
        round_trips,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small but varied corpus: enough repetition for merges to form, with
    /// punctuation, digits, and multi-byte UTF-8 to exercise the byte alphabet.
    fn sample_corpus() -> Vec<String> {
        let base = "the quick brown fox jumps over the lazy dog. \
             the dog was not amused, but the fox ran on and on. \
             tokenization turns text into tokens; tokens turn back into text. \
             123 + 456 = 579, and 2 * 21 = 42 (probably). \
             café, naïve, jalapeño, Москва, 東京, emoji like 🚀 and 🦀 too. ";
        // Repeat so byte pairs clear the min-frequency floor and merges form.
        (0..200).map(|_| base.to_owned()).collect()
    }

    /// Train a small tokenizer in-memory (no corpus file needed) for tests.
    fn train_tiny(vocab_size: usize) -> Tokenizer {
        let mut tokenizer = build_byte_level_bpe();
        let mut trainer = build_trainer(vocab_size, 2, false);
        tokenizer
            .train(&mut trainer, sample_corpus().iter())
            .expect("training succeeds");
        tokenizer
    }

    #[test]
    fn special_token_inventory_is_unique_and_ordered() {
        // PAD must be first so it is pinned to id 0; all entries distinct.
        assert_eq!(SPECIAL_TOKENS[0], PAD_TOKEN);
        let unique: HashSet<_> = SPECIAL_TOKENS.iter().collect();
        assert_eq!(unique.len(), SPECIAL_TOKENS.len());
    }

    #[test]
    fn round_trips_on_arbitrary_utf8() {
        let tokenizer = train_tiny(2_000);
        for text in [
            "hello world",
            "  leading and trailing spaces  ",
            "newlines\nand\ttabs",
            "café Москва 東京 🚀🦀",
            "punctuation!? (yes) — em-dash, 'quotes'",
            "",
        ] {
            assert!(
                round_trips(&tokenizer, text).expect("round-trip check runs"),
                "failed to round-trip {text:?}",
            );
        }
    }

    #[test]
    fn special_tokens_each_encode_atomically() {
        let tokenizer = train_tiny(2_000);
        verify_special_tokens_atomic(&tokenizer).expect("special tokens are atomic");
    }

    #[test]
    fn pad_is_pinned_to_id_zero() {
        let tokenizer = train_tiny(2_000);
        assert_eq!(tokenizer.token_to_id(PAD_TOKEN), Some(0));
    }

    #[test]
    fn trained_vocab_hits_the_requested_size() {
        // A toy corpus only supports a few hundred merges, so the target is set
        // below that ceiling — the trainer then stops *exactly* at the requested
        // size, which is the "vocab size equals the value set" DoD property. On
        // the real CommonPile slice the same holds at the locked 32k.
        let tokenizer = train_tiny(320);
        assert_eq!(tokenizer.get_vocab_size(true), 320);
    }

    #[test]
    fn merges_compress_better_than_the_byte_floor() {
        // A larger vocab merges more, so chars/token must strictly exceed the
        // ~1.0 floor of an unmerged byte-level tokenizer. (We can't hit the
        // 3.5–4 production target on a toy corpus; the real check runs on the
        // CommonPile slice via `wubbie tokenizer`.)
        let tokenizer = train_tiny(2_000);
        let cpt = chars_per_token(&tokenizer, &sample_corpus()[0]).expect("ratio computes");
        assert!(cpt > 1.5, "expected real merging, got {cpt} chars/token");
    }

    #[test]
    fn report_bundles_the_acceptance_checks() {
        // Train at the corpus's merge ceiling so the compression ratio sits
        // comfortably above the byte floor (a low, near-floor vocab leaves
        // chars/token close enough to the threshold that BPE tie-breaking makes
        // the assertion flaky). The exact-size DoD property is covered by
        // `trained_vocab_hits_the_requested_size`.
        let tokenizer = train_tiny(2_000);
        let report = report(&tokenizer, &sample_corpus()[0]).expect("report succeeds");
        assert!(report.vocab_size > 256, "merges should have formed");
        assert!(report.round_trips);
        assert!(report.chars_per_token > 1.5);
    }

    #[test]
    fn vocab_size_below_the_floor_is_rejected() {
        let err = train_from_files(&["unused.txt"], 100, 2)
            .expect_err("a vocab below the byte+special floor is rejected");
        assert!(err.to_string().contains("too small"));
    }

    #[test]
    fn empty_file_list_is_rejected() {
        let no_files: &[&str] = &[];
        let err = train_from_files(no_files, VOCAB_SIZE, 2)
            .expect_err("training needs at least one file");
        assert!(err.to_string().contains("no corpus files"));
    }
}
