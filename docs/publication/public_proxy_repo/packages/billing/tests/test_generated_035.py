"""Generated filler test 035 for the billing package."""

from __future__ import annotations

from billing.generated.generated_035 import build_billing_payload_035


def test_generated_payload_035() -> None:
    payload = build_billing_payload_035("seed")
    assert payload["identifier"].startswith("seed-")
