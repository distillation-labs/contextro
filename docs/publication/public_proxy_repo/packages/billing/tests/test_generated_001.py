"""Generated filler test 001 for the billing package."""

from __future__ import annotations

from billing.generated.generated_001 import build_billing_payload_001


def test_generated_payload_001() -> None:
    payload = build_billing_payload_001("seed")
    assert payload["identifier"].startswith("seed-")
