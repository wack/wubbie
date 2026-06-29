# __SERVICE_NAME__

A Wack microservice built from [`rust-service-template`](https://github.com/wack/rust-service-template).

## Overview

`__SERVICE_NAME__` is a Rust web service using the [Salvo](https://salvo.rs) framework,
[SeaORM](https://www.sea-orm.org) for PostgreSQL persistence, and OpenTelemetry for
observability. It follows a hexagonal (ports-and-adapters) architecture so that business
logic stays free of I/O concerns and integration tests can run against in-memory mocks.

The crate ships:

- An HTTP server exposing the service's REST API plus interactive OpenAPI docs at `/scalar`.
- A code-generated, strongly-typed HTTP client (`__SERVICE_NAME__-client`) derived from
  the service's own OpenAPI specification.
- Database migrations managed with SeaORM.

## Architecture

The codebase is organized into hexagonal layers. Requests flow inward through thin
adapters; the domain core has no framework or I/O dependencies.

| Layer | Directory | Responsibility |
|-------|-----------|----------------|
| Controllers | `src/controllers/` | Thin Salvo HTTP handlers: parse requests, call services, render responses. |
| Services | `src/services/` | Business logic and use cases; coordinate repositories. |
| Repositories | `src/repos/` | Data-access traits (ports) and their SeaORM implementations. |
| Domain | `src/domain/` | Business entities and domain errors. No framework or I/O dependencies. |
| Views | `src/views/` | Serializable HTTP request/response types. |
| Models | `src/models/` | SeaORM entity definitions. |
| Middleware | `src/middleware/` | Salvo middleware (e.g. the JSON error catcher). |

Supporting crates:

- `crates/migrations/` — SeaORM migration definitions.
- `crates/client/` — the generated OpenAPI client (built and validated in CI).

All external I/O sits behind async traits, so production code uses real implementations
while tests inject mocks (see `CLAUDE.md` for the full Sans-I/O design notes).

## Usage

The binary is `__service_name__` and exposes three subcommands:

```bash
# Run the web server
cargo run -- server

# Print the CLI version and exit
cargo run -- version

# Export the OpenAPI specification to stdout
cargo run -- export-openapi > openapi.json
```

Once the server is running, interactive API documentation is available at
`http://127.0.0.1:8080/scalar` (or `https://…` when TLS is configured).

## Configuration

Most flags have an environment-variable equivalent (clap `env`). Common `server` options:

| Flag | Env var | Default | Description |
|------|---------|---------|-------------|
| `--host` | `HOST` | `127.0.0.1` | Bind address. |
| `--port` | `PORT` | `8080` | Bind port. |
| `--pg-host` | `POSTGRES_HOST` | `localhost` | PostgreSQL host. |
| `--postgres-port` | `POSTGRES_PORT` | `5432` | PostgreSQL port. |
| `--database` | `POSTGRES_DATABASE` | `postgres` | Database name. |
| `--username` | `POSTGRES_USER` | _(empty)_ | Database user. |
| `--password` | `POSTGRES_PASSWORD` | _(empty)_ | Database password. |
| `--tls-cert` | `TLS_CERT` | _(none)_ | Path to TLS certificate (requires `--tls-key`). |
| `--tls-key` | `TLS_KEY` | _(none)_ | Path to TLS key (requires `--tls-cert`). |
| `--log-level` | `LOG_LEVEL` | `info` | Tracing level filter. |
| `--log-format` | `LOG_FORMAT` | `text` | `text` or `json` log output. |
| `--deployment-environment` | `OTEL_DEPLOYMENT_ENVIRONMENT` | `local` | OTel resource environment (`development`, `production`, `stable`, `local`). |
| `--otel-exporter-api-key` | `OTEL_EXPORTER_API_KEY` | _(none)_ | OTLP collector API key (`X-API-KEY`). |
| `--service-version` | `SERVICE_VERSION` | crate version | Service version reported to OTel. |

CORS options are also exposed via `--cors-*` flags. Run `cargo run -- server --help` for
the complete, authoritative list.

## Development

Prerequisites: the toolchain is pinned via `rust-toolchain.toml` (Rust 1.96.0); `rustup`
installs it automatically. Most workflows go through [`cargo-make`](https://github.com/sagiegurari/cargo-make).

```bash
# Start a local PostgreSQL instance
cargo make pg

# Apply database migrations
cargo make migrate

# Build the server and write the OpenAPI spec to openapi.json.
# (The __SERVICE_NAME__-client crate is regenerated from openapi.json on the
# next `cargo build` by its build script.)
cargo make generate-openapi

# Watch tests and rerun on change (runs `bacon nextest`)
cargo make bacon

# Or run bare `bacon` for the default fmt-check -> check -> clippy watch chain
bacon

# Strict clippy — the canonical lint command shared by the bacon clippy job and CI.
# (`clippy-strict` is a cargo alias defined in .cargo/config.toml.)
cargo clippy-strict

# Run the test suite
cargo make test

# Check formatting
cargo make check-format
```

## Project Structure

```
.
├── src/
│   ├── bin/main.rs        # Binary entry point
│   ├── lib.rs             # Library root (shared by binary and tests)
│   ├── cli/               # clap subcommands (server, version, export-openapi)
│   ├── controllers/       # HTTP handlers (thin adapters)
│   ├── services/          # Business logic layer
│   ├── repos/             # Repository ports + SeaORM implementations
│   ├── domain/            # Business entities and domain errors
│   ├── views/             # HTTP request/response types
│   ├── models/            # SeaORM entities
│   ├── middleware/        # Salvo middleware
│   └── utils/             # Database, telemetry, and shared helpers
├── crates/
│   ├── migrations/        # SeaORM migrations
│   └── client/            # Generated OpenAPI client (__SERVICE_NAME__-client)
├── helm/chart/            # Helm deployment chart
├── Makefile.toml          # cargo-make task definitions
├── rust-toolchain.toml    # Pinned Rust toolchain
└── Cargo.toml             # Workspace manifest
```
