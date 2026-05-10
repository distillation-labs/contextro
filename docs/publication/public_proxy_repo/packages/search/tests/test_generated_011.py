"""Generated filler test 011 for the search package."""

from __future__ import annotations

from search.generated.generated_011 import build_search_payload_011


def test_generated_payload_011() -> None:
    payload = build_search_payload_011("seed")
    assert payload["identifier"].startswith("seed-")
