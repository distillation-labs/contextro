"""Generated filler test 020 for the search package."""

from __future__ import annotations

from search.generated.generated_020 import build_search_payload_020


def test_generated_payload_020() -> None:
    payload = build_search_payload_020("seed")
    assert payload["identifier"].startswith("seed-")
