"""Generated filler test 013 for the search package."""

from __future__ import annotations

from search.generated.generated_013 import build_search_payload_013


def test_generated_payload_013() -> None:
    payload = build_search_payload_013("seed")
    assert payload["identifier"].startswith("seed-")
