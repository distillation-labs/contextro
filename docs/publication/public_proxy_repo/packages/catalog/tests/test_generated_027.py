"""Generated filler test 027 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_027 import build_catalog_payload_027


def test_generated_payload_027() -> None:
    payload = build_catalog_payload_027("seed")
    assert payload["identifier"].startswith("seed-")
