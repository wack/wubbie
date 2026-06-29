# wubbie

> A fully-open SLM, from corpus to inference.

`wubbie` is the all-Rust pipeline repository for a small language model: the
model definition, the training loop, and the inference server all live here.
It is built on an all-Rust stack:

| Concern    | Crate                                                  |
| ---------- | ------------------------------------------------------ |
| Framework  | [`burn`](https://burn.dev/docs/burn/) (CUDA via CubeCL)|
| Tokenizer  | [`tokenizers`](https://github.com/huggingface/tokenizers) |
| Weights    | [`safetensors`](https://github.com/huggingface/safetensors) |

## Tokenizer

The tokenizer is a GPT-2-style **byte-level BPE** trained with the
[`tokenizers`] crate. Two things are **locked** at training time and feed every
downstream phase, so they live as constants in `src/tokenizer.rs`:

- **Vocabulary size:** `32_000` (`tokenizer::VOCAB_SIZE`). The model config
  sizes its embedding table to this, and the loss-at-init ≈ `ln(vocab)` check is
  taken against it.
- **Special-token inventory** (`tokenizer::SPECIAL_TOKENS`), reserved as atomic
  tokens at fixed low ids — **fixed here and not extendable later**:

  | id | token          | role                       |
  | -- | -------------- | -------------------------- |
  | 0  | `<|pad|>`      | padding / ignore index     |
  | 1  | `<|bos|>`      | beginning of sequence      |
  | 2  | `<|eos|>`      | end of sequence            |
  | 3  | `<|im_start|>` | chat-template turn start   |
  | 4  | `<|im_end|>`   | chat-template turn end     |

  The chat-template *rendering format* is finalized later (Phase 3 SFT, applied
  identically at Phase 6 serving); the tokens themselves exist now.

Train it on a corpus slice with `wubbie tokenizer`:

```bash
# `--input` is a text file or a directory of `.txt` files
cargo run -p wubbie -- tokenizer --input corpus/ --output tokenizer.json
```

After training it runs the acceptance checks against a sample of the corpus:
exact `decode(encode(text)) == text` round-trip, each special token encodes
atomically, and the ~3.5–4 chars/token compression ratio (a miss warns).

[`tokenizers`]: https://github.com/huggingface/tokenizers

Trained weights do **not** live in this repository — they are published to a
separate HuggingFace model repo. This repo holds the code that produces and
serves them.

## Layout

```
.
├── Cargo.toml              # virtual workspace + pinned dependencies
├── crates/
│   └── wubbie/             # the pipeline crate (library + `wubbie` CLI)
│       └── src/
│           ├── lib.rs
│           ├── bin/main.rs # CLI entry point (thin: parse → dispatch)
│           ├── config/     # CLI (clap) layer + model/run configuration
│           ├── cmd/        # subcommand handlers (tokenizer / train / generate / serve)
│           ├── backend.rs  # compile-time backend selection (CPU / CUDA)
│           ├── model.rs    # model definition
│           ├── tokenizer.rs# byte-level BPE tokenizer (train + load)
│           ├── training.rs # training loop
│           ├── inference.rs# inference entry points
│           └── weights.rs  # safetensors (de)serialization
├── Dockerfile              # CPU inference image
└── .github/workflows/on-push.yml
```

## Dependencies

The three pipeline-critical crates are **pinned to exact versions** in the
workspace `[workspace.dependencies]` table, and everything else is locked via
`Cargo.lock`:

- `burn` `=0.21.0`
- `tokenizers` `=0.23.1`
- `safetensors` `=0.8.0`

## Backends

Burn is generic over its compute backend; wubbie selects one at compile time
via crate features:

- **`ndarray`** (default) — a pure-Rust CPU backend that builds everywhere. This
  is what CI builds and the default for `cargo build`.
- **`cuda`** — the NVIDIA CUDA backend via CubeCL, for GPU training/inference.
  It requires the CUDA toolkit at build time and is therefore not part of the
  default build or CI:

  ```bash
  cargo build --release --features cuda
  ```

## Development

```bash
cargo build              # build (CPU backend)
cargo test               # run the test suite
cargo fmt --all          # format
cargo clippy --all-targets --workspace -- -D warnings   # lint (CI gate)
```

If you have [`cargo-make`](https://github.com/sagiegurari/cargo-make)
installed, `cargo make ci` runs the full CI gate (format check → clippy →
build → test) locally.

The `wubbie` CLI exposes four subcommands. `tokenizer` is implemented (see
above); `train`, `generate`, and `serve` are wired up but not yet implemented:

```bash
cargo run -p wubbie -- tokenizer --input corpus/   # train the BPE tokenizer
cargo run -p wubbie -- train
cargo run -p wubbie -- generate "Once upon a time"   # or `-` to read stdin
cargo run -p wubbie -- serve
```

The CLI follows a fixed layout: a thin entrypoint (`src/bin/main.rs`) parses
args and dispatches; each subcommand has an argument struct under `src/config/`
and a handler under `src/cmd/`. New subcommands inherit this structure. Model
and run configuration (`ModelConfig`, the named `ModelSize`s, `TrainingConfig`,
and the reproducible `RunConfig` bundle) also live under `src/config/` and are
`serde`-serializable for reproducible runs.

Configuration is loaded in layers via [`figment`] (`config/loader.rs`). A
`LayeredConfig` builder merges, in increasing precedence, a named-size base, an
optional (possibly partial) config file, `WUBBIE_MODEL_`-prefixed environment
variables, and per-field CLI flags, then extracts a **fully-specified**
`ModelConfig` — every `Option` is resolved or defaulted, and a field left unset
with no default is a hard error rather than a silent `None`. For example:

```bash
# base gpt2-small, with d_model from the file, num_layers from env, d_ff from a flag
WUBBIE_MODEL_NUM_LAYERS=24 \
  cargo run -p wubbie -- train --config model.toml --d-ff 5000
```

[`figment`]: https://docs.rs/figment

## CI

Workflows are named after their trigger event:

- `.github/workflows/on-push.yml` runs on push (PR branches).
- `.github/workflows/on-merge.yml` runs on the GitHub merge queue
  (`merge_group`), if one is enabled.

Both run the same gate:

1. `cargo fmt --all --check`
2. `cargo clippy --all-targets --workspace --locked -- -D warnings`
3. `cargo build --workspace --locked`
4. `cargo test --workspace --locked`

A separate `cuda-build` job compile-checks the CUDA backend
(`cargo build --no-default-features --features cuda`). `cudarc` uses dynamic
loading, so this builds with no GPU, driver, or CUDA toolkit present — it only
validates that the `cuda`-gated code compiles; running it needs a GPU host.
