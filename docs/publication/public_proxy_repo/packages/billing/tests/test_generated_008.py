"""Generated filler test 008 for the billing package."""

from __future__ import annotations

from billing.generated.generated_008 import build_billing_payload_008


def test_generated_payload_008() -> None:
    payload = build_billing_payload_008("seed")
    assert payload["identifier"].startswith("seed-")
