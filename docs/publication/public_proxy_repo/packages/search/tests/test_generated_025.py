"""Generated filler test 025 for the search package."""

from __future__ import annotations

from search.generated.generated_025 import build_search_payload_025


def test_generated_payload_025() -> None:
    payload = build_search_payload_025("seed")
    assert payload["identifier"].startswith("seed-")
