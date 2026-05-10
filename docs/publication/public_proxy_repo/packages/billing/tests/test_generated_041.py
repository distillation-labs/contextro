"""Generated filler test 041 for the billing package."""

from __future__ import annotations

from billing.generated.generated_041 import build_billing_payload_041


def test_generated_payload_041() -> None:
    payload = build_billing_payload_041("seed")
    assert payload["identifier"].startswith("seed-")
