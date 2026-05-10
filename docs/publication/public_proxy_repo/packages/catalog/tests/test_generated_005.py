"""Generated filler test 005 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_005 import build_catalog_payload_005


def test_generated_payload_005() -> None:
    payload = build_catalog_payload_005("seed")
    assert payload["identifier"].startswith("seed-")
