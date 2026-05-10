"""Generated filler test 016 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_016 import build_catalog_payload_016


def test_generated_payload_016() -> None:
    payload = build_catalog_payload_016("seed")
    assert payload["identifier"].startswith("seed-")
