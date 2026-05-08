"""Embedding service with ONNX Runtime support and GPU/MPS auto-detection.

Supported models:
- nomic-embed (768d, 8192 seq len, ONNX) — DEFAULT, fast + high quality, no trust_remote_code
- jina-code (768d, 8192 seq len, ONNX) — code-specific, requires trust_remote_code
- bge-small-en (384d, 512 seq len, PyTorch) — fastest, lightweight
- codesearch-modernbert (768d, 8192 seq len) — highest code search quality
"""

import gc
import hashlib
import logging
import threading
from functools import lru_cache
from typing import Dict, List, Optional

logger = logging.getLogger(__name__)


class _HashEmbeddingModel:
    """Deterministic fallback embedding model used when ONNX loading fails."""

    def __init__(self, dimensions: int, normalize: bool = True):
        self._dimensions = dimensions
        self._normalize = normalize

    def get_sentence_embedding_dimension(self) -> int:
        return self._dimensions

    def encode(self, texts, **kwargs):
        import numpy as np

        if isinstance(texts, str):
            texts = [texts]

        normalize = kwargs.get("normalize_embeddings", self._normalize)
        vectors = []
        for text in texts:
            seed = hashlib.sha256(text.encode("utf-8")).digest()
            values = []
            while len(values) < self._dimensions:
                for byte in seed:
                    values.append((byte / 127.5) - 1.0)
                    if len(values) >= self._dimensions:
                        break
                seed = hashlib.sha256(seed).digest()
            vec = np.asarray(values[: self._dimensions], dtype=np.float32)
            if normalize:
                norm = float(np.linalg.norm(vec))
                if norm > 0:
                    vec = vec / norm
            vectors.append(vec)
        return np.stack(vectors, axis=0)


# Supported embedding models
EMBEDDING_MODELS = {
    "potion-code-16m": {
        "hf_name": "minishlab/potion-code-16M",
        "dimensions": 256,
        "max_seq_length": 512,
        "trust_remote_code": False,
        "prompt_prefix": "",
        "query_prefix": "",
        "backend": "model2vec",
    },
    "potion-8m": {
        "hf_name": "minishlab/potion-base-8M",
        "dimensions": 256,
        "max_seq_length": 512,
        "trust_remote_code": False,
        "prompt_prefix": "",
        "query_prefix": "",
        "backend": "model2vec",
    },
    "nomic-embed": {
        "hf_name": "nomic-ai/nomic-embed-text-v1.5",
        "dimensions": 768,
        "max_seq_length": 8192,
        "trust_remote_code": True,  # nomic-bert custom architecture
        "prompt_prefix": "search_document: ",
        "query_prefix": "search_query: ",
        "backend": "onnx",
    },
    "jina-code": {
        "hf_name": "jinaai/jina-embeddings-v2-base-code",
        "dimensions": 768,
        "max_seq_length": 8192,
        "trust_remote_code": True,
        "prompt_prefix": "",
        "query_prefix": "",
        "backend": "onnx",
    },
    "bge-small-en": {
        "hf_name": "BAAI/bge-small-en-v1.5",
        "dimensions": 384,
        "max_seq_length": 512,
        "trust_remote_code": False,
        "prompt_prefix": "",
        "query_prefix": "Represent this sentence for searching relevant passages: ",
        "backend": None,
    },
    "codesearch-modernbert": {
        "hf_name": "Shuu12121/CodeSearch-ModernBERT-Owl-Plus",
        "dimensions": 768,
        "max_seq_length": 8192,
        "trust_remote_code": False,
        "prompt_prefix": "",
        "query_prefix": "",
        "backend": None,
    },
}

DEFAULT_MODEL = "potion-code-16m"


def _detect_device() -> str:
    """Detect best available device: cuda > mps > cpu."""
    try:
        import torch

        if torch.cuda.is_available():
            return "cuda"
        if hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
            return "mps"
    except (ImportError, AttributeError):
        pass
    try:
        import onnxruntime

        providers = onnxruntime.get_available_providers()
        if "CUDAExecutionProvider" in providers:
            return "cuda"
        if "CoreMLExecutionProvider" in providers:
            return "mps"
    except ImportError:
        pass
    return "cpu"


class EmbeddingService:
    """Embedding service with lazy model loading and batch processing."""

    def __init__(
        self,
        model_name: str = DEFAULT_MODEL,
        device: Optional[str] = None,
        batch_size: int = 32,
        max_batch_size: int = 128,
        normalize: bool = True,
        cache_dir: Optional[str] = None,
    ):
        self.model_name = model_name
        self.batch_size = batch_size
        self.max_batch_size = max_batch_size
        self.normalize = normalize
        self.cache_dir = cache_dir

        if model_name not in EMBEDDING_MODELS:
            from contextro_mcp.core.exceptions import ConfigurationError

            supported = ", ".join(sorted(EMBEDDING_MODELS.keys()))
            raise ConfigurationError(
                f"Unsupported embedding model '{model_name}'. Supported models: {supported}"
            )
        self.config = EMBEDDING_MODELS[model_name].copy()

        self.device = device if device and device != "auto" else _detect_device()
        self._lock = threading.Lock()
        self._model = None
        self._model_loaded = False

        self.stats = {
            "total_embeddings": 0,
            "total_batches": 0,
            "total_time": 0.0,
            "cache_hits": 0,
            "cache_misses": 0,
        }

        self._embed_cached = lru_cache(maxsize=256)(self._embed_single_uncached)

    def _load_model(self):
        if self._model_loaded:
            return
        with self._lock:
            if self._model_loaded:
                return

            backend = self.config.get("backend")

            # Model2Vec: ultra-fast static embeddings (100-500x faster than transformers)
            if backend == "model2vec":
                from model2vec import StaticModel

                self._model = StaticModel.from_pretrained(
                    self.config["hf_name"], force_download=False
                )
                self._model_loaded = True
                self._is_model2vec = True
                logger.info(
                    "Loaded %s with Model2Vec (static embeddings, ~80k emb/sec)",
                    self.config["hf_name"],
                )
                return

            self._is_model2vec = False

            # Check trust_remote_code gate BEFORE importing sentence-transformers
            # so ConfigurationError is raised even when the package isn't installed.
            if self.config.get("trust_remote_code"):
                from contextro_mcp.config import get_settings
                from contextro_mcp.core.exceptions import ConfigurationError

                if not get_settings().trust_remote_code:
                    raise ConfigurationError(
                        f"Model '{self.model_name}' requires trust_remote_code=True, "
                        f"which allows arbitrary code execution from HuggingFace. "
                        f"Set CTX_TRUST_REMOTE_CODE=true to accept this risk."
                    )

            try:
                from sentence_transformers import SentenceTransformer
            except ImportError:
                raise ImportError(
                    "sentence-transformers required. Install: pip install sentence-transformers"
                )

            kwargs = {"device": self.device}
            if self.cache_dir:
                kwargs["cache_folder"] = self.cache_dir
            if self.config.get("trust_remote_code"):
                kwargs["trust_remote_code"] = True

            backend = self.config.get("backend")
            if backend == "onnx":
                import os as _os

                kwargs["backend"] = "onnx"
                if self.device == "cuda":
                    kwargs["model_kwargs"] = {"provider": "CUDAExecutionProvider"}
                elif self.device == "mps":
                    kwargs["model_kwargs"] = {"provider": "CoreMLExecutionProvider"}
                else:
                    # Optimize CPU threading for maximum throughput
                    num_cores = _os.cpu_count() or 4
                    kwargs["model_kwargs"] = {
                        "provider": "CPUExecutionProvider",
                        "provider_options": {},
                        "session_options": {
                            "intra_op_num_threads": num_cores,
                            "inter_op_num_threads": 1,
                        },
                    }
                # ONNX manages its own device; remove SentenceTransformer device param
                kwargs.pop("device", None)
                logger.info(
                    "Loading %s with ONNX Runtime backend (device=%s, cores=%d)",
                    self.config["hf_name"],
                    self.device,
                    _os.cpu_count() or 0,
                )

            import io
            import sys

            old_stdout, old_stderr = sys.stdout, sys.stderr
            sys.stdout, sys.stderr = io.StringIO(), io.StringIO()
            try:
                try:
                    self._model = SentenceTransformer(self.config["hf_name"], **kwargs)
                except Exception as e:
                    if backend == "onnx" and self._should_fallback_from_onnx(e):
                        logger.warning(
                            "Falling back to deterministic hash embeddings for %s: %s",
                            self.config["hf_name"],
                            e,
                        )
                        self._model = _HashEmbeddingModel(
                            self.config["dimensions"],
                            normalize=self.normalize,
                        )
                    else:
                        raise
            finally:
                sys.stdout, sys.stderr = old_stdout, old_stderr

            if self.config["dimensions"] is None:
                self.config["dimensions"] = self._model.get_sentence_embedding_dimension()
            self._model_loaded = True

    @staticmethod
    def _should_fallback_from_onnx(error: Exception) -> bool:
        message = str(error).lower()
        return (
            "optimum.onnxruntime" in message
            or "using the onnx backend requires installing optimum" in message
            or "onnxruntime" in message
            and "installing optimum" in message
        )

    @property
    def dimensions(self) -> int:
        self._load_model()
        return self.config["dimensions"]

    def _embed_single_uncached(self, text: str, is_query: bool) -> tuple:
        self._load_model()
        # Model2Vec path
        if getattr(self, "_is_model2vec", False):
            embeddings = self._model.encode([text])
            return tuple(embeddings[0].tolist())
        # Sentence-transformers path
        prefix = self.config["query_prefix"] if is_query else self.config["prompt_prefix"]
        if prefix:
            text = prefix + text
        embeddings = self._model.encode(
            [text],
            show_progress_bar=False,
            normalize_embeddings=self.normalize,
            convert_to_numpy=True,
        )
        return tuple(embeddings[0].tolist())

    def embed(self, text: str, is_query: bool = False) -> List[float]:
        """Embed single text with LRU caching."""
        info_before = self._embed_cached.cache_info()
        result = self._embed_cached(text, is_query)
        info_after = self._embed_cached.cache_info()
        if info_after.hits > info_before.hits:
            self.stats["cache_hits"] += 1
        else:
            self.stats["cache_misses"] += 1
            self.stats["total_embeddings"] += 1
        return list(result)

    def embed_batch(
        self, texts: List[str], is_query: bool = False, batch_size: Optional[int] = None
    ) -> List[List[float]]:
        """Embed batch of texts."""
        import time

        if not texts:
            return []
        self._load_model()

        start = time.time()

        # Model2Vec path: ultra-fast, no batching needed
        if getattr(self, "_is_model2vec", False):
            embeddings = self._model.encode(texts)
            elapsed = time.time() - start
            self.stats["total_embeddings"] += len(texts)
            self.stats["total_batches"] += 1
            self.stats["total_time"] += elapsed
            return embeddings.tolist()

        # Sentence-transformers path
        bs = min(batch_size or self.batch_size, self.max_batch_size)
        prefix = self.config["query_prefix"] if is_query else self.config["prompt_prefix"]
        if prefix:
            texts = [prefix + t for t in texts]

        embeddings = self._model.encode(
            texts,
            batch_size=bs,
            show_progress_bar=False,
            normalize_embeddings=self.normalize,
            convert_to_numpy=True,
        )
        elapsed = time.time() - start

        self.stats["total_embeddings"] += len(texts)
        self.stats["total_batches"] += (len(texts) + bs - 1) // bs
        self.stats["total_time"] += elapsed

        result = embeddings.tolist()
        del embeddings  # Free numpy array immediately
        return result

    def get_stats(self) -> dict:
        stats = self.stats.copy()
        stats["embeddings_per_second"] = (
            stats["total_embeddings"] / stats["total_time"] if stats["total_time"] > 0 else 0
        )
        return stats

    def unload(self):
        with self._lock:
            if self._model is not None:
                del self._model
                self._model = None
                self._model_loaded = False
                self._embed_cached.cache_clear()
                gc.collect()
                # For model2vec: torch stays resident but we can free its memory pool
                if getattr(self, "_is_model2vec", False):
                    try:
                        import torch

                        if torch.cuda.is_available():
                            torch.cuda.empty_cache()
                        # Force Python to release as much memory as possible
                        gc.collect()
                        gc.collect()  # Two passes for cyclic references
                    except ImportError:
                        pass


# Singleton management
_services: Dict[str, EmbeddingService] = {}
_singleton_lock = threading.Lock()


def get_embedding_service(model_name: str = DEFAULT_MODEL, **kwargs) -> EmbeddingService:
    """Get or create singleton embedding service for a model."""
    if model_name not in _services:
        with _singleton_lock:
            if model_name not in _services:
                _services[model_name] = EmbeddingService(model_name, **kwargs)
    return _services[model_name]


def reset_embedding_service(model_name: Optional[str] = None):
    """Reset embedding service(s)."""
    with _singleton_lock:
        if model_name is None:
            for svc in _services.values():
                svc.unload()
            _services.clear()
        elif model_name in _services:
            _services[model_name].unload()
            del _services[model_name]
