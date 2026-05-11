"""Generated filler test 021 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_021 import build_catalog_payload_021


def test_generated_payload_021() -> None:
    payload = build_catalog_payload_021("seed")
    assert payload["identifier"].startswith("seed-")
