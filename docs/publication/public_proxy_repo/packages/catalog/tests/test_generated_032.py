"""Generated filler test 032 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_032 import build_catalog_payload_032


def test_generated_payload_032() -> None:
    payload = build_catalog_payload_032("seed")
    assert payload["identifier"].startswith("seed-")
