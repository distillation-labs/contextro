"""Generated filler test 002 for the search package."""

from __future__ import annotations

from search.generated.generated_002 import build_search_payload_002


def test_generated_payload_002() -> None:
    payload = build_search_payload_002("seed")
    assert payload["identifier"].startswith("seed-")
