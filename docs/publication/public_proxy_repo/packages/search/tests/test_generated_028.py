"""Generated filler test 028 for the search package."""

from __future__ import annotations

from search.generated.generated_028 import build_search_payload_028


def test_generated_payload_028() -> None:
    payload = build_search_payload_028("seed")
    assert payload["identifier"].startswith("seed-")
