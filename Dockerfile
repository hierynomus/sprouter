# Stage 1: Build the binary using the official Rust image
FROM rust:1.86.0-slim AS builder

# Create a new empty shell project
WORKDIR /usr/src/sprouter

# Pre-copy manifest to cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN touch src/lib.rs
RUN cargo build
RUN rm -rf src

# Copy the real source
COPY src src

# Build for release
RUN cargo build --release

# Stage 2: Create minimal runtime image
FROM debian:bookworm-slim

# Copy compiled binary from builder
COPY --from=builder /usr/src/sprouter/target/release/sprouter /usr/local/bin/sprouter


# Run as non-root user (optional, recommended)
RUN useradd -m suse
USER suse
RUN mkdir -p /home/suse/.kube
COPY rigel.yaml /home/suse/.kube/config

ENTRYPOINT ["/usr/local/bin/sprouter"]
