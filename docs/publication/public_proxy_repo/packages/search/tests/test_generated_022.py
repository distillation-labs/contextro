"""Generated filler test 022 for the search package."""

from __future__ import annotations

from search.generated.generated_022 import build_search_payload_022


def test_generated_payload_022() -> None:
    payload = build_search_payload_022("seed")
    assert payload["identifier"].startswith("seed-")
