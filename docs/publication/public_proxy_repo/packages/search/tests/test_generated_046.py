"""Generated filler test 046 for the search package."""

from __future__ import annotations

from search.generated.generated_046 import build_search_payload_046


def test_generated_payload_046() -> None:
    payload = build_search_payload_046("seed")
    assert payload["identifier"].startswith("seed-")
