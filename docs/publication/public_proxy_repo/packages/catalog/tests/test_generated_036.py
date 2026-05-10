"""Generated filler test 036 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_036 import build_catalog_payload_036


def test_generated_payload_036() -> None:
    payload = build_catalog_payload_036("seed")
    assert payload["identifier"].startswith("seed-")
