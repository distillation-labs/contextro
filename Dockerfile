FROM python:3.12-slim AS builder

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_NO_CACHE_DIR=1 \
    VIRTUAL_ENV=/opt/venv \
    PATH="/opt/venv/bin:$PATH" \
    HF_HOME=/opt/hf-cache

RUN python -m venv "$VIRTUAL_ENV"

# Build deps: compilers for native wheels, git for commit history, ripgrep for live grep fallback.
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates git gcc g++ ripgrep && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY pyproject.toml README.md ./
COPY src ./src
COPY scripts ./scripts

RUN pip install --upgrade pip && \
    pip install --index-url https://download.pytorch.org/whl/cpu torch && \
    pip install -e ".[reranker,model2vec]"

# Pre-cache the default embedding model so first search does not hit Hugging Face.
RUN python -c "from model2vec import StaticModel; StaticModel.from_pretrained('minishlab/potion-code-16M')"


FROM python:3.12-slim AS runtime

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_NO_CACHE_DIR=1 \
    VIRTUAL_ENV=/opt/venv \
    PATH="/opt/venv/bin:$PATH" \
    HF_HOME=/opt/hf-cache \
    CTX_STORAGE_DIR=/data/.contextro \
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
        ca-certificates git ripgrep && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /opt/venv /opt/venv
COPY --from=builder /opt/hf-cache /opt/hf-cache
COPY pyproject.toml README.md ./
COPY src ./src
COPY scripts ./scripts

VOLUME /data
EXPOSE 8000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["python", "/app/scripts/docker_healthcheck.py"]

ENTRYPOINT ["contextro"]
