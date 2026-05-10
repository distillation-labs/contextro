"""Generated filler test 008 for the search package."""

from __future__ import annotations

from search.generated.generated_008 import build_search_payload_008


def test_generated_payload_008() -> None:
    payload = build_search_payload_008("seed")
    assert payload["identifier"].startswith("seed-")
