"""Generated filler test 023 for the search package."""

from __future__ import annotations

from search.generated.generated_023 import build_search_payload_023


def test_generated_payload_023() -> None:
    payload = build_search_payload_023("seed")
    assert payload["identifier"].startswith("seed-")
