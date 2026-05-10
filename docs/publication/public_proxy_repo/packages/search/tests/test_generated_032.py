"""Generated filler test 032 for the search package."""

from __future__ import annotations

from search.generated.generated_032 import build_search_payload_032


def test_generated_payload_032() -> None:
    payload = build_search_payload_032("seed")
    assert payload["identifier"].startswith("seed-")
