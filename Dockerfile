
FROM rust:1-bookworm AS builder

# Install minimal build tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    musl-tools \
    pkg-config \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/dshp

# Copy project
COPY . .

# Build release
RUN cargo build --release

# Strip symbols to reduce size (best-effort)
RUN strip --strip-all target/release/dshp || true

FROM debian:bookworm-slim

# Runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin

# Copy the statically-linked release binary from the builder stage
COPY --from=builder /usr/src/dshp/target/release/dshp .

# Default command
ENTRYPOINT ["/usr/local/bin/dshp"]

