"""Generated filler test 010 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_010 import build_catalog_payload_010


def test_generated_payload_010() -> None:
    payload = build_catalog_payload_010("seed")
    assert payload["identifier"].startswith("seed-")
