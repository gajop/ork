# Build stage
FROM rust:1.83 AS builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml ./

# Copy source code
COPY src ./src
COPY migrations ./migrations

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install SSL certificates and CA certificates for HTTPS
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/rust-orchestrator /app/rust-orchestrator

# Copy migrations
COPY migrations ./migrations

ENV DATABASE_URL=postgres://postgres:postgres@postgres:5432/orchestrator

ENTRYPOINT ["/app/rust-orchestrator"]
