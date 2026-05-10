"""Generated filler test 022 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_022 import build_catalog_payload_022


def test_generated_payload_022() -> None:
    payload = build_catalog_payload_022("seed")
    assert payload["identifier"].startswith("seed-")
