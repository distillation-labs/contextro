"""Generated filler test 047 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_047 import build_catalog_payload_047


def test_generated_payload_047() -> None:
    payload = build_catalog_payload_047("seed")
    assert payload["identifier"].startswith("seed-")
