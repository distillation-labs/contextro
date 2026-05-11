FROM rust:bookworm AS builder

WORKDIR /app/crates

# Native dependencies required by the Rust workspace.
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        clang \
        cmake \
        git \
        libclang-dev \
        libssl-dev \
        make \
        pkg-config && \
    rm -rf /var/lib/apt/lists/*

COPY crates ./

RUN cargo build --locked --release --bin contextro -p contextro-server

FROM debian:bookworm-slim AS runtime

ENV CTX_STORAGE_DIR=/data/.contextro \
    CTX_EMBEDDING_MODEL=potion-code-16m \
    CTX_LOG_LEVEL=INFO \
    CTX_LOG_FORMAT=json \
    CTX_TRANSPORT=http \
    CTX_HTTP_HOST=0.0.0.0 \
    CTX_HTTP_PORT=8000 \
    CTX_SEARCH_CACHE_TTL_SECONDS=300 \
    CTX_SEARCH_SANDBOX_TTL_SECONDS=600 \
    CTX_SEARCH_SANDBOX_THRESHOLD_TOKENS=1200 \
    CTX_SEARCH_PREVIEW_RESULTS=4 \
    CTX_SEARCH_PREVIEW_CODE_CHARS=220

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/crates/target/release/contextro /usr/local/bin/contextro

VOLUME /data
EXPOSE 8000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["curl", "--fail", "--silent", "http://127.0.0.1:8000/health"]

ENTRYPOINT ["contextro"]
