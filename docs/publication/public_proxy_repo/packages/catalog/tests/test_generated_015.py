"""Generated filler test 015 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_015 import build_catalog_payload_015


def test_generated_payload_015() -> None:
    payload = build_catalog_payload_015("seed")
    assert payload["identifier"].startswith("seed-")
