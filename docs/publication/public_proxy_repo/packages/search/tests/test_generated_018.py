"""Generated filler test 018 for the search package."""

from __future__ import annotations

from search.generated.generated_018 import build_search_payload_018


def test_generated_payload_018() -> None:
    payload = build_search_payload_018("seed")
    assert payload["identifier"].startswith("seed-")
