"""Generated filler test 004 for the search package."""

from __future__ import annotations

from search.generated.generated_004 import build_search_payload_004


def test_generated_payload_004() -> None:
    payload = build_search_payload_004("seed")
    assert payload["identifier"].startswith("seed-")
