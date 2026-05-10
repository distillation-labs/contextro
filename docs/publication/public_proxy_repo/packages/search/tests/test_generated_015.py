"""Generated filler test 015 for the search package."""

from __future__ import annotations

from search.generated.generated_015 import build_search_payload_015


def test_generated_payload_015() -> None:
    payload = build_search_payload_015("seed")
    assert payload["identifier"].startswith("seed-")
