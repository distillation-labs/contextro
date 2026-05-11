"""Generated filler test 005 for the search package."""

from __future__ import annotations

from search.generated.generated_005 import build_search_payload_005


def test_generated_payload_005() -> None:
    payload = build_search_payload_005("seed")
    assert payload["identifier"].startswith("seed-")
