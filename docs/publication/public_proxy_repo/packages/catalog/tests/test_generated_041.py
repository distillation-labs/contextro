"""Generated filler test 041 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_041 import build_catalog_payload_041


def test_generated_payload_041() -> None:
    payload = build_catalog_payload_041("seed")
    assert payload["identifier"].startswith("seed-")
