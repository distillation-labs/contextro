"""Generated filler test 049 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_049 import build_catalog_payload_049


def test_generated_payload_049() -> None:
    payload = build_catalog_payload_049("seed")
    assert payload["identifier"].startswith("seed-")
