# Build stage
FROM rust:1.89-bookworm AS builder

WORKDIR /build

# Install RocksDB dependencies
RUN apt-get update && apt-get install -y \
    librocksdb-dev \
    libclang-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY core/blockchain/ core/blockchain/
COPY wallet/core/ wallet/core/

# Build release binary
RUN cargo build --release --bin isa-chain-node

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    librocksdb-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/isa-chain-node /usr/local/bin/

# Configuration
ENV CHAIN_ID=15489
ENV RPC_PORT=9944
ENV BLOCK_TIME_SECS=3
ENV DATA_DIR=/data
ENV PERSIST=true
ENV RUST_LOG=isa_chain=info

EXPOSE 9944

VOLUME ["/data"]

ENTRYPOINT ["isa-chain-node"]
