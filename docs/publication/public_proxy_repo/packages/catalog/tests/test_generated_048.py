"""Generated filler test 048 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_048 import build_catalog_payload_048


def test_generated_payload_048() -> None:
    payload = build_catalog_payload_048("seed")
    assert payload["identifier"].startswith("seed-")
