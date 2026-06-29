# Build stage
FROM rust:1.92-alpine AS builder

# Install build dependencies (no OpenSSL since we use rustls)
# git is required for git-fetch-with-cli (private GitHub dependencies)
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    git

# Add the musl target for static compilation (native architecture)
RUN rustup target add $(uname -m)-unknown-linux-musl

# Set the working directory
WORKDIR /app

# Build-time provenance for OpenTelemetry resource attributes. Defaults are
# empty so local `docker build` (without --build-arg) falls through to the
# "unknown" branch in build.rs and the image still builds. CI is expected
# to pass real values via `docker/build-push-action`'s `build-args` input.
ARG VCS_REF_HEAD_REVISION=""
ARG VCS_REF_HEAD_NAME=""
ARG VCS_REF_HEAD_TYPE=""
ARG VCS_REPOSITORY_URL_FULL=""
ARG CICD_PIPELINE_NAME=""
ARG CICD_PIPELINE_RUN_URL_FULL=""
ENV VCS_REF_HEAD_REVISION=${VCS_REF_HEAD_REVISION} \
    VCS_REF_HEAD_NAME=${VCS_REF_HEAD_NAME} \
    VCS_REF_HEAD_TYPE=${VCS_REF_HEAD_TYPE} \
    VCS_REPOSITORY_URL_FULL=${VCS_REPOSITORY_URL_FULL} \
    CICD_PIPELINE_NAME=${CICD_PIPELINE_NAME} \
    CICD_PIPELINE_RUN_URL_FULL=${CICD_PIPELINE_RUN_URL_FULL}

# Copy cargo config early (needed for git-fetch-with-cli setting)
COPY .cargo/config.toml .cargo/config.toml

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY crates/migrations/Cargo.toml ./crates/migrations/
COPY crates/client/Cargo.toml ./crates/client/

# Create dummy source files to cache dependencies
# Uses BuildKit secret mount for private GitHub dependency authentication.
# The git config is written and cleaned up within the same layer to avoid leaking the token.
RUN --mount=type=secret,id=GITHUB_TOKEN \
    if [ -f /run/secrets/GITHUB_TOKEN ]; then \
      git config --global url."https://x-access-token:$(cat /run/secrets/GITHUB_TOKEN)@github.com/".insteadOf "https://github.com/"; \
      git config --global --add url."https://x-access-token:$(cat /run/secrets/GITHUB_TOKEN)@github.com/".insteadOf "ssh://git@github.com/"; \
    fi && \
    mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    mkdir -p src/bin && \
    echo "fn main() {}" > src/bin/main.rs && \
    mkdir -p crates/migrations/src && \
    echo "use sea_orm_migration::prelude::*; #[derive(DeriveMigrationName)] pub struct Migration; #[async_trait::async_trait] impl MigrationTrait for Migration { async fn up(&self, _: &SchemaManager) -> Result<(), DbErr> { Ok(()) } async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> { Ok(()) } }" > crates/migrations/src/lib.rs && \
    echo "fn main() {}" > crates/migrations/src/main.rs && \
    mkdir -p crates/client/src && \
    echo "fn main() {}" > crates/client/build.rs && \
    echo "" > crates/client/src/lib.rs && \
    cargo build --release --target $(uname -m)-unknown-linux-musl && \
    rm -rf src crates && \
    rm -f /root/.gitconfig

# Copy actual source code
COPY . .

# Build the application statically
# Touch source files to ensure they rebuild after the dummy
RUN touch src/bin/main.rs crates/migrations/src/lib.rs && \
    cargo build --release --target $(uname -m)-unknown-linux-musl && \
    cp /app/target/$(uname -m)-unknown-linux-musl/release/__SERVICE_NAME__ /app-binary

# Runtime stage
FROM gcr.io/distroless/static:nonroot

# Copy the binary from builder
COPY --from=builder /app-binary /__SERVICE_NAME__

# Set the working directory
WORKDIR /

# Production defaults for logging and telemetry
ENV LOG_FORMAT=json
ENV LOG_LEVEL=info
ENV OTEL_DEPLOYMENT_ENVIRONMENT=production

# Expose the port the server listens on
EXPOSE 8080

# Run the binary with the server subcommand by default
ENTRYPOINT ["/__SERVICE_NAME__"]
CMD ["server"]
