FROM rust:1.83

# Install OpenSSL and pkg-config (needed for building with OpenSSL)
RUN apt-get update && \
    apt-get install -y \
    pkg-config \
    libssl-dev \
    mingw-w64 \
    gcc-mingw-w64 \
    binutils-mingw-w64 \
    g++-mingw-w64 \
    --no-install-recommends

# Install cross
RUN cargo install cross
