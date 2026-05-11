"""Generated filler test 043 for the search package."""

from __future__ import annotations

from search.generated.generated_043 import build_search_payload_043


def test_generated_payload_043() -> None:
    payload = build_search_payload_043("seed")
    assert payload["identifier"].startswith("seed-")
