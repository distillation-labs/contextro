"""Generated filler test 014 for the billing package."""

from __future__ import annotations

from billing.generated.generated_014 import build_billing_payload_014


def test_generated_payload_014() -> None:
    payload = build_billing_payload_014("seed")
    assert payload["identifier"].startswith("seed-")
