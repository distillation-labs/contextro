"""Generated filler test 007 for the billing package."""

from __future__ import annotations

from billing.generated.generated_007 import build_billing_payload_007


def test_generated_payload_007() -> None:
    payload = build_billing_payload_007("seed")
    assert payload["identifier"].startswith("seed-")
