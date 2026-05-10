"""Generated filler test 037 for the billing package."""

from __future__ import annotations

from billing.generated.generated_037 import build_billing_payload_037


def test_generated_payload_037() -> None:
    payload = build_billing_payload_037("seed")
    assert payload["identifier"].startswith("seed-")
