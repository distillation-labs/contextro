"""Generated filler test 011 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_011 import build_catalog_payload_011


def test_generated_payload_011() -> None:
    payload = build_catalog_payload_011("seed")
    assert payload["identifier"].startswith("seed-")
