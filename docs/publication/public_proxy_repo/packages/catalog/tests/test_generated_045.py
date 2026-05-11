"""Generated filler test 045 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_045 import build_catalog_payload_045


def test_generated_payload_045() -> None:
    payload = build_catalog_payload_045("seed")
    assert payload["identifier"].startswith("seed-")
