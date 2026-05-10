"""Generated filler test 036 for the search package."""

from __future__ import annotations

from search.generated.generated_036 import build_search_payload_036


def test_generated_payload_036() -> None:
    payload = build_search_payload_036("seed")
    assert payload["identifier"].startswith("seed-")
