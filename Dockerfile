# Use the official Rust image as the base
FROM rust:1.83 AS builder

# Set the working directory
WORKDIR /app

# Copy the Cargo.toml and Cargo.lock
COPY Cargo.toml ./

# Copy the source code
COPY . .

# Build the application in release mode
RUN cargo build --release

# Use a smaller image for the final application
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libpq-dev

# Set the working directory
WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/ppdrive .

# Set the default command to run the application
CMD ["./ppdrive"]
