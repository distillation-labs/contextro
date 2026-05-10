"""Generated filler test 021 for the search package."""

from __future__ import annotations

from search.generated.generated_021 import build_search_payload_021


def test_generated_payload_021() -> None:
    payload = build_search_payload_021("seed")
    assert payload["identifier"].startswith("seed-")
