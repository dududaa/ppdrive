FROM rust:1.83 AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libpq-dev \
    libssl3 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -r ppdrive && useradd -r -g ppdrive -d /app ppdrive

WORKDIR /app

COPY --from=builder /app/target/release/ppdrive /usr/local/bin/ppdrive
COPY --from=builder /app/target/release/server /usr/local/bin/server
COPY --from=builder /app/migrations ./migrations

RUN chown -R ppdrive:ppdrive /app

USER ppdrive

EXPOSE 8000

HEALTHCHECK --interval=30s --timeout=3s \
    CMD curl -f http://localhost:8000/health || exit 1

CMD ["server"]
