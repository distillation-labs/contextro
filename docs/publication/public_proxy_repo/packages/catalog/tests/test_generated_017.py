"""Generated filler test 017 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_017 import build_catalog_payload_017


def test_generated_payload_017() -> None:
    payload = build_catalog_payload_017("seed")
    assert payload["identifier"].startswith("seed-")
