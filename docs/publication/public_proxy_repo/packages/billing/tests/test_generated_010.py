"""Generated filler test 010 for the billing package."""

from __future__ import annotations

from billing.generated.generated_010 import build_billing_payload_010


def test_generated_payload_010() -> None:
    payload = build_billing_payload_010("seed")
    assert payload["identifier"].startswith("seed-")
