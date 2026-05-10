"""Generated filler test 024 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_024 import build_catalog_payload_024


def test_generated_payload_024() -> None:
    payload = build_catalog_payload_024("seed")
    assert payload["identifier"].startswith("seed-")
