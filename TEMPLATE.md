# Service Template

This is a production-ready Rust microservice template built with the Salvo web framework, SeaORM, and PostgreSQL. It follows hexagonal architecture principles and includes everything needed to build, test, and deploy a microservice.

## Quick Start

### 1. Initialize Your Service

Run the initialization script to replace placeholders with your service name:

```bash
./scripts/init-service.sh my-service
```

Or run interactively:

```bash
./scripts/init-service.sh
```

The script will:
- Replace `__SERVICE_NAME__` with your service name (e.g., `my-service`)
- Replace `__service_name__` with the Rust crate name (e.g., `my_service`)
- Update all configuration files, Helm charts, and source code

### 2. Start Development Database

```bash
cargo make pg
```

### 3. Run Migrations

```bash
cargo make migrate
```

### 4. Build and Run

```bash
cargo run -- server
```

The server will start at `http://127.0.0.1:8080` with OpenAPI docs at `/scalar`.

## What's Included

### Project Structure

```
├── src/
│   ├── bin/main.rs          # Application entry point
│   ├── lib.rs               # Library exports
│   ├── cli/                 # CLI configuration (clap)
│   ├── controllers/         # HTTP handlers (thin adapters)
│   ├── services/            # Business logic layer
│   ├── repos/               # Repository abstractions
│   ├── domain/              # Domain entities and logic
│   ├── views/               # HTTP request/response types
│   ├── models/              # SeaORM entities
│   ├── middleware/          # Salvo middleware
│   ├── db/                  # Database connection pool
│   └── utils/               # Utilities (telemetry, etc.)
├── crates/
│   ├── migrations/          # SeaORM database migrations
│   └── client/              # Generated API client (optional)
├── helm/
│   └── chart/               # Kubernetes Helm chart
├── .github/workflows/       # CI/CD pipelines
├── Dockerfile               # Multi-stage Docker build
└── Makefile.toml            # Development tasks
```

### Architecture

This template follows **hexagonal architecture** (ports and adapters):

1. **Domain Layer** (`src/domain/`) - Business entities with no external dependencies
2. **Service Layer** (`src/services/`) - Business logic and use cases
3. **Repository Layer** (`src/repos/`) - Data access abstraction
4. **View Layer** (`src/views/`) - HTTP request/response types
5. **Controller Layer** (`src/controllers/`) - Thin HTTP adapters

### Features

- **Web Framework**: Salvo with OpenAPI/Swagger support
- **Database**: PostgreSQL with SeaORM async ORM
- **Observability**: OpenTelemetry tracing, structured logging
- **Security**: TLS support, non-root containers, security contexts
- **Deployment**: Docker, Helm charts, GitHub Actions CI/CD
- **Developer Experience**: cargo-make tasks, hot reload ready

### Available Commands

| Command | Description |
|---------|-------------|
| `cargo make pg` | Start PostgreSQL in Docker |
| `cargo make migrate` | Run database migrations |
| `cargo make migrate-down` | Rollback migrations |
| `cargo make generate-entities` | Generate SeaORM entities from DB |
| `cargo make generate-openapi` | Export OpenAPI spec |
| `cargo make test` | Run test suite |
| `cargo make ci-flow` | Run full CI pipeline |
| `cargo make bacon` | Watch mode testing |

### CLI Commands

```bash
# Run the server
cargo run -- server

# Print version
cargo run -- version

# Export OpenAPI spec
cargo run -- export-openapi > openapi.json
```

## Customization Guide

### Adding a New Entity

1. **Create migration** in `crates/migrations/src/`
2. **Run migration**: `cargo make migrate`
3. **Generate entities**: `cargo make generate-entities`
4. **Create domain types** in `src/domain/`
5. **Create repository trait** in `src/repos/`
6. **Implement service** in `src/services/`
7. **Create views** in `src/views/`
8. **Add controller** in `src/controllers/`
9. **Register routes** in `src/controllers/mod.rs`

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `HOST` | Server host | `127.0.0.1` |
| `PORT` | Server port | `8080` |
| `DATABASE_URL` | Full PostgreSQL URL | - |
| `POSTGRES_HOST` | Database host | - |
| `POSTGRES_PORT` | Database port | `5432` |
| `POSTGRES_USER` | Database user | - |
| `POSTGRES_PASSWORD` | Database password | - |
| `POSTGRES_DATABASE` | Database name | - |
| `LOG_LEVEL` | Log level | `info` |
| `LOG_FORMAT` | `text` or `json` | `text` |
| `TLS_CERT` | TLS certificate path | - |
| `TLS_KEY` | TLS key path | - |
| `CORS_ALLOWED_ORIGINS` | Comma-separated origins | - |

### Helm Deployment

```bash
# Install
helm install my-service helm/chart \
  --set database.host=postgres.example.com \
  --set database.user=myuser \
  --set database.password=mypassword \
  --set database.name=mydb

# Upgrade
helm upgrade my-service helm/chart -f values-prod.yaml
```

## CI/CD Pipeline

The GitHub Actions workflows provide:

1. **on-push.yml** - Format check on feature branches
2. **on-merge.yml** - Full validation on merge queue:
   - Rust formatting and linting
   - Build and test with nextest
   - Docker image build and push to GHCR
   - Helm chart validation and publish

Images are tagged with the commit SHA and pushed to:
- `ghcr.io/<org>/<service-name>:<sha>`

## Removing Template Files

After initialization, you can remove template-specific files:
- `TEMPLATE.md` (this file)
- `scripts/init-service.sh`

The initialization script offers to remove these automatically.

## License

This template is provided under the MIT License.
