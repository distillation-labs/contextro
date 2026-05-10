"""Generated filler test 050 for the search package."""

from __future__ import annotations

from search.generated.generated_050 import build_search_payload_050


def test_generated_payload_050() -> None:
    payload = build_search_payload_050("seed")
    assert payload["identifier"].startswith("seed-")
