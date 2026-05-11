"""Generated filler test 007 for the search package."""

from __future__ import annotations

from search.generated.generated_007 import build_search_payload_007


def test_generated_payload_007() -> None:
    payload = build_search_payload_007("seed")
    assert payload["identifier"].startswith("seed-")
