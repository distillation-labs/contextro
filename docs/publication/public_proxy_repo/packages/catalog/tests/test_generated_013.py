"""Generated filler test 013 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_013 import build_catalog_payload_013


def test_generated_payload_013() -> None:
    payload = build_catalog_payload_013("seed")
    assert payload["identifier"].startswith("seed-")
