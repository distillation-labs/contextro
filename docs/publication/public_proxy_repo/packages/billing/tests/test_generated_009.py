"""Generated filler test 009 for the billing package."""

from __future__ import annotations

from billing.generated.generated_009 import build_billing_payload_009


def test_generated_payload_009() -> None:
    payload = build_billing_payload_009("seed")
    assert payload["identifier"].startswith("seed-")
