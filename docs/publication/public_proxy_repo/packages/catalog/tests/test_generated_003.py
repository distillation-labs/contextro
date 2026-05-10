"""Generated filler test 003 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_003 import build_catalog_payload_003


def test_generated_payload_003() -> None:
    payload = build_catalog_payload_003("seed")
    assert payload["identifier"].startswith("seed-")
