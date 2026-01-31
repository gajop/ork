# Build stage (dependency-cached via cargo-chef)
FROM lukemathwalker/cargo-chef:latest-rust-1.93 AS chef

WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
COPY . .
RUN cargo build -p ork-cli --release

# Runtime stage (distroless includes CA certs; no apt-get needed)
FROM gcr.io/distroless/cc-debian12:nonroot

WORKDIR /app

# Copy the binary from builder
COPY --from=builder --chown=65532:65532 /app/target/release/ork /app/ork

# Copy migrations
COPY --from=builder --chown=65532:65532 /app/crates/ork-cli/migrations ./migrations

ENV DATABASE_URL=postgres://postgres:postgres@postgres:5432/orchestrator

ENTRYPOINT ["/app/ork"]
