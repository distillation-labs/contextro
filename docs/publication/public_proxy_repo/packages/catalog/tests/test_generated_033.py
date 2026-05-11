"""Generated filler test 033 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_033 import build_catalog_payload_033


def test_generated_payload_033() -> None:
    payload = build_catalog_payload_033("seed")
    assert payload["identifier"].startswith("seed-")
