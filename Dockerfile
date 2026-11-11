# Stage 1: Build the release binary
FROM rust:1.78-slim AS builder

WORKDIR /app

# Copy all project files
COPY . .

# Build the release binary
RUN cargo build --release

# Stage 2: Runtime environment
FROM debian:bookworm-slim AS runtime

# Install TLS certificates for outbound HTTPS calls (e.g. Stellar Horizon API)
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy release binary from builder
COPY --from=builder /app/target/release/stellar-tipchain-backend /usr/local/bin/stellar-tipchain-backend

# Expose API port
EXPOSE 8000

ENV PORT=8000

CMD ["stellar-tipchain-backend"]
