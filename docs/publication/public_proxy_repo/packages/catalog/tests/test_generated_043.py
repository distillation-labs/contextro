"""Generated filler test 043 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_043 import build_catalog_payload_043


def test_generated_payload_043() -> None:
    payload = build_catalog_payload_043("seed")
    assert payload["identifier"].startswith("seed-")
