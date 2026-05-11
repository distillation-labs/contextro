"""Generated filler test 041 for the search package."""

from __future__ import annotations

from search.generated.generated_041 import build_search_payload_041


def test_generated_payload_041() -> None:
    payload = build_search_payload_041("seed")
    assert payload["identifier"].startswith("seed-")
