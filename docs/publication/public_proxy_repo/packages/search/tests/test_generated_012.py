"""Generated filler test 012 for the search package."""

from __future__ import annotations

from search.generated.generated_012 import build_search_payload_012


def test_generated_payload_012() -> None:
    payload = build_search_payload_012("seed")
    assert payload["identifier"].startswith("seed-")
