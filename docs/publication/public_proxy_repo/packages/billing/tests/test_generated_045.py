"""Generated filler test 045 for the billing package."""

from __future__ import annotations

from billing.generated.generated_045 import build_billing_payload_045


def test_generated_payload_045() -> None:
    payload = build_billing_payload_045("seed")
    assert payload["identifier"].startswith("seed-")
