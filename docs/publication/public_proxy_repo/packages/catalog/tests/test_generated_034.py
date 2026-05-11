"""Generated filler test 034 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_034 import build_catalog_payload_034


def test_generated_payload_034() -> None:
    payload = build_catalog_payload_034("seed")
    assert payload["identifier"].startswith("seed-")
