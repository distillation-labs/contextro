"""Generated filler test 039 for the search package."""

from __future__ import annotations

from search.generated.generated_039 import build_search_payload_039


def test_generated_payload_039() -> None:
    payload = build_search_payload_039("seed")
    assert payload["identifier"].startswith("seed-")
