"""Generated filler test 007 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_007 import build_catalog_payload_007


def test_generated_payload_007() -> None:
    payload = build_catalog_payload_007("seed")
    assert payload["identifier"].startswith("seed-")
