"""Generated filler test 048 for the search package."""

from __future__ import annotations

from search.generated.generated_048 import build_search_payload_048


def test_generated_payload_048() -> None:
    payload = build_search_payload_048("seed")
    assert payload["identifier"].startswith("seed-")
