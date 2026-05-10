"""Generated filler test 031 for the search package."""

from __future__ import annotations

from search.generated.generated_031 import build_search_payload_031


def test_generated_payload_031() -> None:
    payload = build_search_payload_031("seed")
    assert payload["identifier"].startswith("seed-")
