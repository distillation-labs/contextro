"""Generated filler test 006 for the search package."""

from __future__ import annotations

from search.generated.generated_006 import build_search_payload_006


def test_generated_payload_006() -> None:
    payload = build_search_payload_006("seed")
    assert payload["identifier"].startswith("seed-")
