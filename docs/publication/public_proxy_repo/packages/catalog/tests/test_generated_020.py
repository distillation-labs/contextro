"""Generated filler test 020 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_020 import build_catalog_payload_020


def test_generated_payload_020() -> None:
    payload = build_catalog_payload_020("seed")
    assert payload["identifier"].startswith("seed-")
