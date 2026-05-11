"""Generated filler test 019 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_019 import build_catalog_payload_019


def test_generated_payload_019() -> None:
    payload = build_catalog_payload_019("seed")
    assert payload["identifier"].startswith("seed-")
