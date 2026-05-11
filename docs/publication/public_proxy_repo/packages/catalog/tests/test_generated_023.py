"""Generated filler test 023 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_023 import build_catalog_payload_023


def test_generated_payload_023() -> None:
    payload = build_catalog_payload_023("seed")
    assert payload["identifier"].startswith("seed-")
