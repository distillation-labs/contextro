"""Generated filler test 014 for the search package."""

from __future__ import annotations

from search.generated.generated_014 import build_search_payload_014


def test_generated_payload_014() -> None:
    payload = build_search_payload_014("seed")
    assert payload["identifier"].startswith("seed-")
