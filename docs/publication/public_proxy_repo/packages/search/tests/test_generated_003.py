"""Generated filler test 003 for the search package."""

from __future__ import annotations

from search.generated.generated_003 import build_search_payload_003


def test_generated_payload_003() -> None:
    payload = build_search_payload_003("seed")
    assert payload["identifier"].startswith("seed-")
