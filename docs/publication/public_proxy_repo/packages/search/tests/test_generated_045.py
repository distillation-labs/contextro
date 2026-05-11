"""Generated filler test 045 for the search package."""

from __future__ import annotations

from search.generated.generated_045 import build_search_payload_045


def test_generated_payload_045() -> None:
    payload = build_search_payload_045("seed")
    assert payload["identifier"].startswith("seed-")
