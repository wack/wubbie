# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

This repository is a production-ready Rust microservice **template** built on the Salvo web
framework, SeaORM, and PostgreSQL. It follows hexagonal (ports-and-adapters) architecture
with a Sans-I/O core, and ships Docker, Helm, and GitHub Actions CI. New services are created
by running `scripts/init-service.sh`, which substitutes the `__SERVICE_NAME__` (kebab) and
`__service_name__` (snake/crate) placeholders.

## Build Commands

```bash
cargo make pg                  # Start a local PostgreSQL (postgres:18-alpine) in Docker
cargo make migrate             # Apply SeaORM migrations (crates/migrations)
cargo run -- server            # Run the web server (OpenAPI docs at /scalar)
cargo clippy-strict            # Canonical lint gate — == CI and the bacon clippy job
cargo make test                # Run the test suite (nextest)
cargo make bacon               # Watch tests and rerun on change (bacon nextest)
cargo make monitor             # Headless bacon for Claude Code's Monitor tool
cargo make coverage            # Test suite under llvm-cov
cargo make generate-openapi    # Build the server and write openapi.json
cargo make generate-entities   # Generate SeaORM entities from the database
cargo make generate-schema     # Dump the post-migration schema to tern/schema.sql
cargo make check-format        # Formatting check (CI runs this)
```

`cargo clippy-strict` is an alias (defined in `.cargo/config.toml`) for `cargo clippy
--all-targets --workspace -- -D warnings` — the single canonical lint surface shared
byte-for-byte by CI, the bacon clippy job, and local runs, so all three agree on pass/fail.
Lint levels live in `[workspace.lints]` (root `Cargo.toml`); `-D warnings` makes any
violation a hard failure. CI runs format, clippy, and OpenAPI generation on PR branches
(`on-push.yml`); the full build, tests, coverage, Docker, and Helm publish run in the merge
queue (`on-merge.yml`).

## Story / Session Tracking

This repo tracks tickets, issues, and handovers under `.story/` (storybloq). All
`.story/` changes for a unit of work belong in **the same commit as that work** — do
not split project-tracking state from the code it describes.

- **Flip ticket status with the work, not after the PR lands.** When you finish a
  ticket, set its status to `complete` in the final commit of that session. Do **not**
  defer closure to a separate post-merge "Close T-XXX" commit, and do **not** leave a
  ticket `inprogress` pending the PR merging. (This supersedes the older land-then-close
  practice some prior handovers describe.)
- **Write the session handover before the final commit.** At the end of a session,
  take a snapshot, write the handover, flip any ticket statuses, and stage all of it
  together — then make the single final commit and open the PR. The handover and status
  changes ship inside that commit / PR, not afterward.
- Only `.story/` files git already tracks are committed; `.story/sessions/`,
  `.story/snapshots/`, and `.story/status.json` are gitignored (ephemeral local state).

## Architecture

Hexagonal (ports and adapters). Requests flow inward through thin adapters; the domain core
has no framework or I/O dependencies.

| Layer | Directory | Responsibility |
|-------|-----------|----------------|
| Controllers | `src/controllers/` | Thin Salvo HTTP handlers: parse requests, call services, render responses. |
| Services | `src/services/` | Business logic and use cases; coordinate repositories. |
| Repositories | `src/repos/` | Data-access traits (ports) and their SeaORM implementations. |
| Domain | `src/domain/` | Business entities and domain errors. No framework/I/O dependencies. |
| Views | `src/views/` | Serializable HTTP request/response types. |
| Models | `src/models/` | SeaORM entity definitions. |
| Middleware | `src/middleware/` | Salvo middleware (CORS, OTel, metrics, store injection). |

**Sans-I/O rule:** business logic never performs I/O directly. Every external dependency
(database, HTTP clients, message queues) sits behind an async trait (port). Production code
uses real implementations; tests inject mocks (`src/middleware/mocks.rs`), so integration
tests run fast and deterministically with no real I/O.

Per-layer conventions are documented in the directory-local CLAUDE.md files:
`src/cli/CLAUDE.md`, `src/controllers/CLAUDE.md`, `src/views/CLAUDE.md`.

## Adding a New Entity

1. **Create a migration** in `crates/migrations/src/` (add it to the migrator's list).
2. **Apply it:** `cargo make migrate`.
3. **Generate entities:** `cargo make generate-entities` (writes to `src/models/`).
4. **Domain types** in `src/domain/<resource>/` — value objects via `nutype`.
5. **Repository trait + impl** in `src/repos/<resource>/` (port + SeaORM adapter).
6. **Service** in `src/services/<resource>/` — business logic over the repo trait.
7. **Views** in `src/views/<resource>/` — request/response types (see `src/views/CLAUDE.md`).
8. **Controller** in `src/controllers/<resource>/` (see `src/controllers/CLAUDE.md`).
9. **Register routes** in `src/controllers/mod.rs`.

Each REST resource gets its own directory; nested resources mirror the URL path hierarchy
(e.g. `workspaces/{id}/api-keys` → `controllers/workspaces/api_keys/`).

## Telemetry

OpenTelemetry tracing + HTTP metrics. See [OPENTELEMETRY.md](OPENTELEMETRY.md) for the
background reference. The router middleware order (`root()` in `src/controllers/mod.rs`),
outermost → innermost, is:

1. `OtelHttpMiddleware` — custom OTel HTTP spans (replaces Salvo's `Logger`)
2. CORS — `create_cors_middleware`
3. `Metrics` — Salvo built-in metrics
4. `metrics_middleware` — custom HTTP status-code histogram
5. `transaction_middleware` — database transaction
6. `item_store_middleware` — store injection

Telemetry is initialized in `src/utils/telemetry.rs`, called from `Server::dispatch`
(`src/cli/server/mod.rs`); the custom middleware lives in `src/middleware/`.

## Configuration

Most CLI flags have an environment-variable equivalent (clap `env`). The most common are
`HOST`/`PORT`, the `POSTGRES_*` connection settings, `TLS_CERT`/`TLS_KEY`, `LOG_LEVEL`,
`LOG_FORMAT`, and the `OTEL_*` exporter settings. README.md documents the common `server`
options; run `cargo run -- server --help` for the complete, authoritative list.

## CI / Infrastructure

- **`on-push.yml`** (PR branches; excludes `trunk` and `gh-readonly-queue/**`): `format`,
  `clippy` (`cargo make clippy-flow`), and `generate-openapi` jobs, gated by `⚡ PR Ready`.
  A concurrency block cancels superseded push runs.
- **`on-merge.yml`** (merge queue): `format`, `helm-lint`, `build` (`cargo make ci-flow`),
  `coverage`, multi-arch `docker`, `manifest`, and Helm publish — all gated by `⚡ PR Ready`.
  `build` and `coverage` share the `rust-ci` cargo cache key.
- **Docker:** multi-stage Alpine build, musl static linking, `distroless/static:nonroot`
  runtime (`Dockerfile`).
- **Helm:** chart in `helm/chart/`; CI sets the chart version to `YYYY.MM.DD-<run_number>`
  and `appVersion` to the commit SHA.

## Agent Guardrails

- **Clippy ratchet:** `cargo clippy-strict` must stay green. Lint levels live in
  `[workspace.lints]`; to adopt a stricter lint, fix the findings it surfaces and land that
  as its own small diff — do not add blanket `allow`s without a rationale comment.
- **Sans-I/O:** never perform I/O in services/domain — depend on a trait (port) and inject
  the implementation.
- **Resource-per-directory:** one directory per REST resource under
  `controllers/`/`services/`/`repos/`/`views/`; nested resources mirror the URL path.
- **Story discipline:** flip ticket status in the same commit as the work; ship `.story/`
  changes with the code they describe.

## Project Structure

```
.
├── src/
│   ├── bin/main.rs        # Binary entry point
│   ├── lib.rs             # Library root
│   ├── cli/               # clap subcommands (server, version, export-openapi) + config
│   ├── controllers/       # HTTP handlers (thin adapters)
│   ├── services/          # Business logic layer
│   ├── repos/             # Repository ports + SeaORM implementations
│   ├── domain/            # Business entities and domain errors
│   ├── views/             # HTTP request/response types
│   ├── models/            # SeaORM entities
│   ├── middleware/        # Salvo middleware (cors, otel_http, metrics, stores, mocks)
│   └── utils/             # db.rs (connection), telemetry.rs (OTel), mod.rs
├── crates/
│   ├── migrations/        # SeaORM migrations (crate name: migration)
│   └── client/            # Generated OpenAPI client (__SERVICE_NAME__-client)
├── helm/chart/            # Helm deployment chart
├── .github/workflows/     # on-push.yml, on-merge.yml
├── Dockerfile             # Multi-stage musl/distroless build
├── Makefile.toml          # cargo-make task definitions
├── rust-toolchain.toml    # Pinned Rust toolchain
└── Cargo.toml             # Workspace manifest + [workspace.lints]
```
