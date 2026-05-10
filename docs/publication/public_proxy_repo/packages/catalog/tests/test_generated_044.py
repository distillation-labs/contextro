"""Generated filler test 044 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_044 import build_catalog_payload_044


def test_generated_payload_044() -> None:
    payload = build_catalog_payload_044("seed")
    assert payload["identifier"].startswith("seed-")
