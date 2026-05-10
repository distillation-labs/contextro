"""Shared token counting helpers backed by tiktoken."""

from __future__ import annotations

import json
from functools import lru_cache

TOKENIZER_LIBRARY = "tiktoken"
DEFAULT_TOKENIZER_ENCODING = "cl100k_base"


@lru_cache(maxsize=1)
def _get_encoder():
    try:
        import tiktoken
    except ImportError as exc:
        raise RuntimeError(
            "tiktoken is required for real token counting. Install project dependencies "
            "or run `pip install tiktoken`."
        ) from exc
    return tiktoken.get_encoding(DEFAULT_TOKENIZER_ENCODING)


def tokenizer_metadata() -> dict[str, str]:
    return {
        "library": TOKENIZER_LIBRARY,
        "encoding": DEFAULT_TOKENIZER_ENCODING,
    }


def serialize_for_token_count(value: object, *, pretty: bool = False) -> str:
    if isinstance(value, str):
        return value
    if pretty:
        return json.dumps(value, indent=2, default=str)
    return json.dumps(value, default=str, separators=(",", ":"))


def count_text_tokens(text: str) -> int:
    if not text:
        return 0
    return len(_get_encoder().encode_ordinary(text))


def truncate_text_to_tokens(text: str, token_limit: int) -> str:
    if token_limit <= 0 or not text:
        return ""
    encoder = _get_encoder()
    return encoder.decode(encoder.encode_ordinary(text)[:token_limit])


def count_serialized_tokens(value: object, *, pretty: bool = False) -> int:
    return count_text_tokens(serialize_for_token_count(value, pretty=pretty))
