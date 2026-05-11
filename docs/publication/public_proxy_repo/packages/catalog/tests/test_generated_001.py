"""Generated filler test 001 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_001 import build_catalog_payload_001


def test_generated_payload_001() -> None:
    payload = build_catalog_payload_001("seed")
    assert payload["identifier"].startswith("seed-")
