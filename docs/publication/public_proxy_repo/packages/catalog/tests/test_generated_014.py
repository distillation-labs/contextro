"""Generated filler test 014 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_014 import build_catalog_payload_014


def test_generated_payload_014() -> None:
    payload = build_catalog_payload_014("seed")
    assert payload["identifier"].startswith("seed-")
