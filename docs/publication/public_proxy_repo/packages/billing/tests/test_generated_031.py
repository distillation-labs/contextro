"""Generated filler test 031 for the billing package."""

from __future__ import annotations

from billing.generated.generated_031 import build_billing_payload_031


def test_generated_payload_031() -> None:
    payload = build_billing_payload_031("seed")
    assert payload["identifier"].startswith("seed-")
