"""Generated filler test 044 for the search package."""

from __future__ import annotations

from search.generated.generated_044 import build_search_payload_044


def test_generated_payload_044() -> None:
    payload = build_search_payload_044("seed")
    assert payload["identifier"].startswith("seed-")
