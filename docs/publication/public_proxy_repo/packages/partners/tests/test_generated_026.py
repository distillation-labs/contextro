"""Generated filler test 026 for the partners package."""

from __future__ import annotations

from partners.generated.generated_026 import build_partners_payload_026


def test_generated_payload_026() -> None:
    payload = build_partners_payload_026("seed")
    assert payload["identifier"].startswith("seed-")
