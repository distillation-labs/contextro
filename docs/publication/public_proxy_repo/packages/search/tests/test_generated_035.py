"""Generated filler test 035 for the search package."""

from __future__ import annotations

from search.generated.generated_035 import build_search_payload_035


def test_generated_payload_035() -> None:
    payload = build_search_payload_035("seed")
    assert payload["identifier"].startswith("seed-")
