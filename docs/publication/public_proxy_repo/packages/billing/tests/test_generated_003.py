"""Generated filler test 003 for the billing package."""

from __future__ import annotations

from billing.generated.generated_003 import build_billing_payload_003


def test_generated_payload_003() -> None:
    payload = build_billing_payload_003("seed")
    assert payload["identifier"].startswith("seed-")
