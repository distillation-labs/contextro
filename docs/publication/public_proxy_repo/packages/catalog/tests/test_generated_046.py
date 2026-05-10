"""Generated filler test 046 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_046 import build_catalog_payload_046


def test_generated_payload_046() -> None:
    payload = build_catalog_payload_046("seed")
    assert payload["identifier"].startswith("seed-")
