"""Generated filler test 019 for the search package."""

from __future__ import annotations

from search.generated.generated_019 import build_search_payload_019


def test_generated_payload_019() -> None:
    payload = build_search_payload_019("seed")
    assert payload["identifier"].startswith("seed-")
