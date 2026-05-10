"""Generated filler test 021 for the billing package."""

from __future__ import annotations

from billing.generated.generated_021 import build_billing_payload_021


def test_generated_payload_021() -> None:
    payload = build_billing_payload_021("seed")
    assert payload["identifier"].startswith("seed-")
