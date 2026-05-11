"""Generated filler test 029 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_029 import build_catalog_payload_029


def test_generated_payload_029() -> None:
    payload = build_catalog_payload_029("seed")
    assert payload["identifier"].startswith("seed-")
