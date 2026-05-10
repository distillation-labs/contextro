"""Generated filler test 017 for the search package."""

from __future__ import annotations

from search.generated.generated_017 import build_search_payload_017


def test_generated_payload_017() -> None:
    payload = build_search_payload_017("seed")
    assert payload["identifier"].startswith("seed-")
