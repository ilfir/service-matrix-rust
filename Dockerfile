FROM rust:1.98-bookworm AS builder

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
ARG SERVICE_MATRIX_GIT_SHA=unknown
ENV SERVICE_MATRIX_GIT_SHA=${SERVICE_MATRIX_GIT_SHA}
RUN cargo build --locked --release

FROM debian:bookworm-slim AS runtime

RUN useradd --system --uid 10001 --create-home service-matrix
WORKDIR /app
COPY --from=builder /src/target/release/service-matrix-rust /usr/local/bin/service-matrix-rust
COPY data ./data
COPY resources ./resources
RUN chown -R service-matrix:service-matrix /app/data /app/resources

USER service-matrix
ENV SERVICE_MATRIX_IN_CONTAINER=1 \
    SERVICE_MATRIX_PORT=8080 \
    RUST_LOG=info
EXPOSE 8080
VOLUME ["/app/data", "/app/resources"]
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD ["/usr/local/bin/service-matrix-rust", "--healthcheck"]
ENTRYPOINT ["/usr/local/bin/service-matrix-rust"]

