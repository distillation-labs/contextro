"""Generated filler test 002 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_002 import build_catalog_payload_002


def test_generated_payload_002() -> None:
    payload = build_catalog_payload_002("seed")
    assert payload["identifier"].startswith("seed-")
