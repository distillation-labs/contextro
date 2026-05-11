"""Generated filler test 030 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_030 import build_catalog_payload_030


def test_generated_payload_030() -> None:
    payload = build_catalog_payload_030("seed")
    assert payload["identifier"].startswith("seed-")
