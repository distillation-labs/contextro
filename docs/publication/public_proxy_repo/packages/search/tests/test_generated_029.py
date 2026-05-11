"""Generated filler test 029 for the search package."""

from __future__ import annotations

from search.generated.generated_029 import build_search_payload_029


def test_generated_payload_029() -> None:
    payload = build_search_payload_029("seed")
    assert payload["identifier"].startswith("seed-")
