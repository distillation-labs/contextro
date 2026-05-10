"""Generated filler test 018 for the billing package."""

from __future__ import annotations

from billing.generated.generated_018 import build_billing_payload_018


def test_generated_payload_018() -> None:
    payload = build_billing_payload_018("seed")
    assert payload["identifier"].startswith("seed-")
