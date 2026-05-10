"""Generated filler test 037 for the search package."""

from __future__ import annotations

from search.generated.generated_037 import build_search_payload_037


def test_generated_payload_037() -> None:
    payload = build_search_payload_037("seed")
    assert payload["identifier"].startswith("seed-")
