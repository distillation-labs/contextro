"""Generated filler test 023 for the billing package."""

from __future__ import annotations

from billing.generated.generated_023 import build_billing_payload_023


def test_generated_payload_023() -> None:
    payload = build_billing_payload_023("seed")
    assert payload["identifier"].startswith("seed-")
