"""Generated filler test 026 for the catalog package."""

from __future__ import annotations

from catalog.generated.generated_026 import build_catalog_payload_026


def test_generated_payload_026() -> None:
    payload = build_catalog_payload_026("seed")
    assert payload["identifier"].startswith("seed-")
