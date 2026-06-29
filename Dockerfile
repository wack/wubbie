# Multi-stage build for the wubbie CLI (CPU / ndarray backend).
#
# This builds the default (CPU) backend so the image runs anywhere. A CUDA image
# (built with `--features cuda` on an NVIDIA base) is a follow-up; see the
# project README.

FROM rust:1.96-bookworm AS builder
WORKDIR /build

# `tokenizers` pulls in the `onig` regex backend, whose build script needs a C
# toolchain and libclang for bindgen.
RUN apt-get update \
    && apt-get install -y --no-install-recommends clang libclang-dev \
    && rm -rf /var/lib/apt/lists/*

COPY . .
RUN cargo build --release --locked -p wubbie

FROM debian:bookworm-slim AS runtime
RUN useradd --create-home --user-group --uid 10001 wubbie
COPY --from=builder /build/target/release/wubbie /usr/local/bin/wubbie
USER wubbie
ENTRYPOINT ["wubbie"]
