"""Generated filler test 013 for the billing package."""

from __future__ import annotations

from billing.generated.generated_013 import build_billing_payload_013


def test_generated_payload_013() -> None:
    payload = build_billing_payload_013("seed")
    assert payload["identifier"].startswith("seed-")
