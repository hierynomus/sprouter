# Create minimal runtime image
# using the SUSE Application Collection BCI Micro image
FROM registry.suse.com/bci/bci-nano:16.0

ARG TARGETARCH

# Copy the pre-compiled binary for the target architecture
COPY --from=binaries linux/${TARGETARCH}/sprouter /usr/local/bin/sprouter

# Run as non-root user (optional, recommended)
USER 1001

ENTRYPOINT ["/usr/local/bin/sprouter"]
