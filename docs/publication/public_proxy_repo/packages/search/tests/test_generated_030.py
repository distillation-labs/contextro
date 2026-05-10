"""Generated filler test 030 for the search package."""

from __future__ import annotations

from search.generated.generated_030 import build_search_payload_030


def test_generated_payload_030() -> None:
    payload = build_search_payload_030("seed")
    assert payload["identifier"].startswith("seed-")
