"""Generated filler test 040 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_040 import build_catalog_payload_040


def test_generated_payload_040() -> None:
    payload = build_catalog_payload_040("seed")
    assert payload["identifier"].startswith("seed-")
