"""Generated filler test 040 for the search package."""

from __future__ import annotations

from search.generated.generated_040 import build_search_payload_040


def test_generated_payload_040() -> None:
    payload = build_search_payload_040("seed")
    assert payload["identifier"].startswith("seed-")
