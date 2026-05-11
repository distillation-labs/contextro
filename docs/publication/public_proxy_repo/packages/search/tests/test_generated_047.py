"""Generated filler test 047 for the search package."""

from __future__ import annotations

from search.generated.generated_047 import build_search_payload_047


def test_generated_payload_047() -> None:
    payload = build_search_payload_047("seed")
    assert payload["identifier"].startswith("seed-")
