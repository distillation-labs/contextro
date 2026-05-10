"""Generated filler test 024 for the search package."""

from __future__ import annotations

from search.generated.generated_024 import build_search_payload_024


def test_generated_payload_024() -> None:
    payload = build_search_payload_024("seed")
    assert payload["identifier"].startswith("seed-")
