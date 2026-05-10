"""Generated filler test 025 for the billing package."""

from __future__ import annotations

from billing.generated.generated_025 import build_billing_payload_025


def test_generated_payload_025() -> None:
    payload = build_billing_payload_025("seed")
    assert payload["identifier"].startswith("seed-")
