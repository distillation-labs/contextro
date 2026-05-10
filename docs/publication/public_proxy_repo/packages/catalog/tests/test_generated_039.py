"""Generated filler test 039 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_039 import build_catalog_payload_039


def test_generated_payload_039() -> None:
    payload = build_catalog_payload_039("seed")
    assert payload["identifier"].startswith("seed-")
