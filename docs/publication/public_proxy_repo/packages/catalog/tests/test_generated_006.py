"""Generated filler test 006 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_006 import build_catalog_payload_006


def test_generated_payload_006() -> None:
    payload = build_catalog_payload_006("seed")
    assert payload["identifier"].startswith("seed-")
