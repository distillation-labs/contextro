"""Generated filler test 033 for the search package."""

from __future__ import annotations

from search.generated.generated_033 import build_search_payload_033


def test_generated_payload_033() -> None:
    payload = build_search_payload_033("seed")
    assert payload["identifier"].startswith("seed-")
