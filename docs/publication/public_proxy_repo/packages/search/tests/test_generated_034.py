"""Generated filler test 034 for the search package."""

from __future__ import annotations

from search.generated.generated_034 import build_search_payload_034


def test_generated_payload_034() -> None:
    payload = build_search_payload_034("seed")
    assert payload["identifier"].startswith("seed-")
