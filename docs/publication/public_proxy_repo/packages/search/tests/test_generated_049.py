"""Generated filler test 049 for the search package."""

from __future__ import annotations

from search.generated.generated_049 import build_search_payload_049


def test_generated_payload_049() -> None:
    payload = build_search_payload_049("seed")
    assert payload["identifier"].startswith("seed-")
