"""Generated filler test 035 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_035 import build_catalog_payload_035


def test_generated_payload_035() -> None:
    payload = build_catalog_payload_035("seed")
    assert payload["identifier"].startswith("seed-")
