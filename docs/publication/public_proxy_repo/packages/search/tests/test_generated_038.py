"""Generated filler test 038 for the search package."""

from __future__ import annotations

from search.generated.generated_038 import build_search_payload_038


def test_generated_payload_038() -> None:
    payload = build_search_payload_038("seed")
    assert payload["identifier"].startswith("seed-")
