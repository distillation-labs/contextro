"""Generated filler test 012 for the billing package."""

from __future__ import annotations

from billing.generated.generated_012 import build_billing_payload_012


def test_generated_payload_012() -> None:
    payload = build_billing_payload_012("seed")
    assert payload["identifier"].startswith("seed-")
