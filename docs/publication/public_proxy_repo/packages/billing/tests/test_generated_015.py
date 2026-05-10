"""Generated filler test 015 for the billing package."""

from __future__ import annotations

from billing.generated.generated_015 import build_billing_payload_015


def test_generated_payload_015() -> None:
    payload = build_billing_payload_015("seed")
    assert payload["identifier"].startswith("seed-")
