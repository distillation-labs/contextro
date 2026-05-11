"""Generated filler test 038 for the partners package."""

from __future__ import annotations

from partners.generated.generated_038 import build_partners_payload_038


def test_generated_payload_038() -> None:
    payload = build_partners_payload_038("seed")
    assert payload["identifier"].startswith("seed-")
