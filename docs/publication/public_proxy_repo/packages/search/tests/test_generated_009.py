"""Generated filler test 009 for the search package."""

from __future__ import annotations

from search.generated.generated_009 import build_search_payload_009


def test_generated_payload_009() -> None:
    payload = build_search_payload_009("seed")
    assert payload["identifier"].startswith("seed-")
