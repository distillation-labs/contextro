"""Generated filler test 002 for the billing package."""

from __future__ import annotations

from billing.generated.generated_002 import build_billing_payload_002


def test_generated_payload_002() -> None:
    payload = build_billing_payload_002("seed")
    assert payload["identifier"].startswith("seed-")
