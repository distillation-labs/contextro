"""Generated filler test 018 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_018 import build_catalog_payload_018


def test_generated_payload_018() -> None:
    payload = build_catalog_payload_018("seed")
    assert payload["identifier"].startswith("seed-")
