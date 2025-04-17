# Stage 1: Build the binary using the official Rust image
FROM registry.suse.com/bci/rust:1.85 AS builder

# Create a new empty shell project
WORKDIR /usr/src/sprouter

# Pre-copy manifest to cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch
COPY src src

# Build for release
RUN cargo build --release

# Stage 2: Create minimal runtime image
FROM registry.suse.com/bci/bci-minimal:15.7

# Copy compiled binary from builder
COPY --from=builder /usr/src/sprouter/target/release/sprouter /usr/local/bin/sprouter

# Run as non-root user (optional, recommended)
USER 1001

ENTRYPOINT ["/usr/local/bin/sprouter"]
