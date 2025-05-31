# Use the official Rust image as the base
FROM rust:1.83 AS builder

# Set the working directory
WORKDIR /app

# Copy the Cargo.toml, Cargo.lock and default config
COPY Cargo.toml Cargo.lock ./

# Copy the source code
COPY . .

# Build the application in release mode
RUN cargo build --release

# Use a smaller image for the final application
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
libpq-dev \
libssl3 \
ca-certificates

# Set the working directory
WORKDIR /app

# Copy release files
COPY --from=builder /app/target/release/ppdrive .
COPY --from=builder /app/core/migrations/ .
COPY --from=builder /app/ppd_config.toml .


# Set the default command to run the application
CMD ["./ppdrive"]
