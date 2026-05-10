"""Generated filler test 016 for the search package."""

from __future__ import annotations

from search.generated.generated_016 import build_search_payload_016


def test_generated_payload_016() -> None:
    payload = build_search_payload_016("seed")
    assert payload["identifier"].startswith("seed-")
