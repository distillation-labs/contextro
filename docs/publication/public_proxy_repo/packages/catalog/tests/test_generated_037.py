"""Generated filler test 037 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_037 import build_catalog_payload_037


def test_generated_payload_037() -> None:
    payload = build_catalog_payload_037("seed")
    assert payload["identifier"].startswith("seed-")
