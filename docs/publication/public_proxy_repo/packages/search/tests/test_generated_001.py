"""Generated filler test 001 for the search package."""

from __future__ import annotations

from search.generated.generated_001 import build_search_payload_001


def test_generated_payload_001() -> None:
    payload = build_search_payload_001("seed")
    assert payload["identifier"].startswith("seed-")
