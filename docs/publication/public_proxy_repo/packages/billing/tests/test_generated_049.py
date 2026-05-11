"""Generated filler test 049 for the billing package."""

from __future__ import annotations

from billing.generated.generated_049 import build_billing_payload_049


def test_generated_payload_049() -> None:
    payload = build_billing_payload_049("seed")
    assert payload["identifier"].startswith("seed-")
