# Use the official Rust image as the base
FROM rust:1.83 AS builder

# Set the working directory
WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./

# Copy the source code
COPY . .

# Build both binaries in release mode
RUN cargo build --release

# Use a smaller image for the final application
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libpq-dev \
    libssl3 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN groupadd -r ppdrive && useradd -r -g ppdrive -d /app ppdrive

# Set the working directory
WORKDIR /app

# Copy release binaries and runtime files
COPY --from=builder /app/target/release/ppdrive /usr/local/bin/ppdrive
COPY --from=builder /app/target/release/server /usr/local/bin/ppdrive-server
COPY --from=builder /app/migrations ./migrations
COPY --from=builder /app/ppd_config.toml .

# Set ownership
RUN chown -R ppdrive:ppdrive /app

# Run as non-root
USER ppdrive

EXPOSE 8000

HEALTHCHECK --interval=30s --timeout=3s \
    CMD curl -f http://localhost:8000/health || exit 1

CMD ["ppdrive-server"]
