"""Generated filler test 044 for the billing package."""

from __future__ import annotations

from billing.generated.generated_044 import build_billing_payload_044


def test_generated_payload_044() -> None:
    payload = build_billing_payload_044("seed")
    assert payload["identifier"].startswith("seed-")
