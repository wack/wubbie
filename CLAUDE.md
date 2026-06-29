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
            ├── bin/main.rs # CLI entry point (thin: parse → dispatch)
            ├── config/     # clap CLI layer (cli/command + per-subcommand args)
            │               #   + model/run config (ModelConfig, RunConfig)
            ├── cmd/        # subcommand handlers (download / tokenizer / train / …)
            ├── backend.rs  # compile-time backend selection (ndarray / cuda)
            ├── corpus.rs   # corpus access: HF (hf-hub) / local; JSONL(.gz) + text
            ├── model.rs    # model definition
            ├── tokenizer.rs# byte-level BPE tokenizer: train + load (tokenizers crate)
            ├── training.rs # training loop
            ├── inference.rs# inference entry points
            └── weights.rs  # safetensors (de)serialization
```

The CLI follows the house convention (mirroring the `multitool` layout): a thin
`src/bin/main.rs` parses args and dispatches; `src/config/` holds the clap layer
(`cli.rs`, `command.rs`, one `<Sub>Subcommand` args struct per subcommand) plus
the `serde`-serializable model/run config; `src/cmd/` holds one handler per
subcommand (`new(args)` + `dispatch()`). New subcommands inherit this structure.

## Dependency Policy

The three pipeline-critical crates are **pinned to exact versions** (`=x.y.z`) in
`[workspace.dependencies]`; everything else is locked transitively via `Cargo.lock` (committed).
When bumping `burn`, `tokenizers`, or `safetensors`, update the exact pin and the README version
table together.

- `burn` `=0.21.0`, `tokenizers` `=0.23.1`, `safetensors` `=0.8.0`.
- Member crates inherit deps with `{ workspace = true }` rather than redeclaring versions.

## Backends

Burn is backend-generic; the concrete backend is chosen at compile time in `backend.rs` via
crate features. Each backend pairs with the precision it ships with:

- **`ndarray`** (default) — pure-Rust CPU backend, builds everywhere. **f32.** This is what CI
  builds.
- **`cuda`** — NVIDIA CUDA backend via CubeCL, the cloud-training path. **bf16** (the locked
  cloud recipe — see MULTI-1386). Requires the CUDA toolkit at build time, so it is intentionally
  **excluded from the default build and CI**. Code touching the CUDA backend is
  `#[cfg(feature = "cuda")]`-gated; it is not compiled by `cargo build`/CI, so verify it builds on
  a CUDA host before relying on it.
- **`wgpu`** — cross-platform WGPU backend via CubeCL (Metal on macOS, Vulkan on Linux/Windows,
  DirectX 12 on Windows), the local-development path. **f32** (Metal does not implement bf16
  arithmetic via WGPU). Not part of the default build or CI; actually running the
  forward/backward path needs a GPU host. CubeCL's `#[cube]` macros require the crate-root
  `#![recursion_limit = "256"]` set in `lib.rs`.

Training uses `backend::TrainBackend` (an `Autodiff`-wrapped backend); inference uses
`backend::Backend` directly.

## CI / Agent Guardrails

- **CI workflows are named after their trigger event.** `.github/workflows/on-push.yml` runs on
  push (PR branches; excludes `trunk` and the merge queue) and `.github/workflows/on-merge.yml`
  runs on `merge_group` (the GitHub merge queue, if enabled). Both run the same jobs: `validate`
  (`cargo fmt --all --check`, `cargo clippy --all-targets --workspace --locked -- -D warnings`,
  `cargo build --workspace --locked`, `cargo test --workspace --locked`) and `cuda-build`, which
  compile-checks the `cuda` feature (`cargo build --no-default-features --features cuda`) — `cudarc`
  uses dynamic loading so it builds with no GPU/toolkit, but running it needs a GPU host. Both
  workflows expose a single gate job named **`⚡ PR Ready`**; keep that name identical across the two
  files so one branch-protection check covers both contexts. It must stay green.
- **`--locked`:** CI builds/tests with `--locked`, so keep `Cargo.lock` committed and in sync.
- **Clippy is the ratchet:** the workspace lints (`[workspace.lints]`) deny `unsafe_code` and warn
  on `clippy::all`, escalated to a hard failure by `-D warnings`. Fix findings rather than adding
  blanket `allow`s.
- **Toolchain** is pinned in `rust-toolchain.toml`.
