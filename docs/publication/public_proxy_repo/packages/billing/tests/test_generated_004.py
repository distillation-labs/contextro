"""Generated filler test 004 for the billing package."""

from __future__ import annotations

from billing.generated.generated_004 import build_billing_payload_004


def test_generated_payload_004() -> None:
    payload = build_billing_payload_004("seed")
    assert payload["identifier"].startswith("seed-")
