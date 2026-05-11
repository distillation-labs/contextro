"""Generated filler test 050 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_050 import build_catalog_payload_050


def test_generated_payload_050() -> None:
    payload = build_catalog_payload_050("seed")
    assert payload["identifier"].startswith("seed-")
