"""Generated filler test 042 for the search package."""

from __future__ import annotations

from search.generated.generated_042 import build_search_payload_042


def test_generated_payload_042() -> None:
    payload = build_search_payload_042("seed")
    assert payload["identifier"].startswith("seed-")
