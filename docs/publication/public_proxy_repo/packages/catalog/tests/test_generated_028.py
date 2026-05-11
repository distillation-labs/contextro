"""Generated filler test 028 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_028 import build_catalog_payload_028


def test_generated_payload_028() -> None:
    payload = build_catalog_payload_028("seed")
    assert payload["identifier"].startswith("seed-")
