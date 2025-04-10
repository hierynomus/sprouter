# Stage 1: Build the binary using the official Rust image
FROM rust:1.86.0-slim as builder

# Create a new empty shell project
WORKDIR /usr/src/shadower

# Pre-copy manifest to cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy the real source
COPY . .

# Build for release
RUN cargo build --release

# Stage 2: Create minimal runtime image
FROM registry.suse.com/bci/bci-base:latest

# Copy compiled binary from builder
COPY --from=builder /usr/src/shadower/target/release/shadower /usr/local/bin/shadower

# Run as non-root user (optional, recommended)
RUN useradd -m suse
USER suse

ENTRYPOINT ["/usr/local/bin/shadower"]
