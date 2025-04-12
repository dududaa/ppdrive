FROM rust:1.83

# Install OpenSSL and pkg-config (needed for building with OpenSSL)
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev libpq-dev

# Install cross
RUN cargo install cross
