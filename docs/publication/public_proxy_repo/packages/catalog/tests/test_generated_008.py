"""Generated filler test 008 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_008 import build_catalog_payload_008


def test_generated_payload_008() -> None:
    payload = build_catalog_payload_008("seed")
    assert payload["identifier"].startswith("seed-")
