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
│           ├── main.rs     # CLI entry point (train / generate / serve)
│           ├── backend.rs  # compile-time backend selection (CPU / CUDA)
│           ├── config.rs   # model configuration
│           ├── model.rs    # model definition
│           ├── tokenizer.rs# tokenizer loading
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

The `wubbie` CLI scaffolds three subcommands; they are wired up but not yet
implemented:

```bash
cargo run -p wubbie -- train
cargo run -p wubbie -- generate
cargo run -p wubbie -- serve
```

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
