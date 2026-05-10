"""Generated filler test 010 for the search package."""

from __future__ import annotations

from search.generated.generated_010 import build_search_payload_010


def test_generated_payload_010() -> None:
    payload = build_search_payload_010("seed")
    assert payload["identifier"].startswith("seed-")
