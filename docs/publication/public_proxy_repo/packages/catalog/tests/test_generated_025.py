"""Generated filler test 025 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_025 import build_catalog_payload_025


def test_generated_payload_025() -> None:
    payload = build_catalog_payload_025("seed")
    assert payload["identifier"].startswith("seed-")
