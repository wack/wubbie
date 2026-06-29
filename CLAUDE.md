# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this
repository.

`wubbie` is the all-Rust pipeline repository for a fully-open small language model: the model
definition, the training loop, and the inference server live here. Trained weights do **not**
live here — they are published to a separate HuggingFace model repo.

The stack is all-Rust: [Burn](https://burn.dev/docs/burn/) (CUDA via CubeCL) for the model
and training, the [`tokenizers`](https://github.com/huggingface/tokenizers) crate for the
tokenizer, and [`safetensors`](https://github.com/huggingface/safetensors) for weights.

## Build Commands

```bash
cargo build                                              # build (CPU/ndarray backend)
cargo test --workspace                                   # run the test suite
cargo fmt --all                                          # format
cargo clippy --all-targets --workspace -- -D warnings    # lint (the CI gate)
cargo build --release --features cuda                    # build the CUDA backend (needs CUDA)
cargo run -p wubbie -- <train|generate|serve>            # run the CLI
```

If [`cargo-make`](https://github.com/sagiegurari/cargo-make) is installed, `cargo make ci`
reproduces the full CI gate locally (format check → clippy → build → test). `cargo clippy-strict`
(alias in `.cargo/config.toml`) is the canonical lint surface — `clippy --all-targets --workspace
-- -D warnings`, identical to CI.

## Workspace Layout

This is a Cargo virtual workspace. All code lives in the single `wubbie` crate today; new crates
are added as members in the root `Cargo.toml`.

```
.
├── Cargo.toml              # virtual workspace + [workspace.dependencies] (pinned)
└── crates/
    └── wubbie/             # pipeline crate: library + `wubbie` binary
        └── src/
            ├── lib.rs      # module roots
            ├── main.rs     # CLI entry point (clap)
            ├── backend.rs  # compile-time backend selection (ndarray / cuda)
            ├── config.rs   # ModelConfig
            ├── model.rs    # model definition
            ├── tokenizer.rs# tokenizer loading (tokenizers crate)
            ├── training.rs # training loop
            ├── inference.rs# inference entry points
            └── weights.rs  # safetensors (de)serialization
```

## Dependency Policy

The three pipeline-critical crates are **pinned to exact versions** (`=x.y.z`) in
`[workspace.dependencies]`; everything else is locked transitively via `Cargo.lock` (committed).
When bumping `burn`, `tokenizers`, or `safetensors`, update the exact pin and the README version
table together.

- `burn` `=0.21.0`, `tokenizers` `=0.23.1`, `safetensors` `=0.8.0`.
- Member crates inherit deps with `{ workspace = true }` rather than redeclaring versions.

## Backends

Burn is backend-generic; the concrete backend is chosen at compile time in `backend.rs` via
crate features:

- **`ndarray`** (default) — pure-Rust CPU backend, builds everywhere. This is what CI builds.
- **`cuda`** — NVIDIA CUDA backend via CubeCL. Requires the CUDA toolkit at build time, so it is
  intentionally **excluded from the default build and CI**. Code touching the CUDA backend is
  `#[cfg(feature = "cuda")]`-gated; it is not compiled by `cargo build`/CI, so verify it builds on
  a CUDA host before relying on it.

Training uses `backend::TrainBackend` (an `Autodiff`-wrapped backend); inference uses
`backend::Backend` directly.

## CI / Agent Guardrails

- **`.github/workflows/ci.yml`** runs on every push and pull request: `cargo fmt --all --check`,
  `cargo clippy --all-targets --workspace --locked -- -D warnings`, `cargo build --workspace
  --locked`, `cargo test --workspace --locked`. It must stay green.
- **`--locked`:** CI builds/tests with `--locked`, so keep `Cargo.lock` committed and in sync.
- **Clippy is the ratchet:** the workspace lints (`[workspace.lints]`) deny `unsafe_code` and warn
  on `clippy::all`, escalated to a hard failure by `-D warnings`. Fix findings rather than adding
  blanket `allow`s.
- **Toolchain** is pinned in `rust-toolchain.toml`.
