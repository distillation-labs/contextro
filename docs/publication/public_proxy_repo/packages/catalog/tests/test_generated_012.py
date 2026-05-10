"""Generated filler test 012 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_012 import build_catalog_payload_012


def test_generated_payload_012() -> None:
    payload = build_catalog_payload_012("seed")
    assert payload["identifier"].startswith("seed-")
