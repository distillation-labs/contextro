"""Generated filler test 038 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_038 import build_catalog_payload_038


def test_generated_payload_038() -> None:
    payload = build_catalog_payload_038("seed")
    assert payload["identifier"].startswith("seed-")
