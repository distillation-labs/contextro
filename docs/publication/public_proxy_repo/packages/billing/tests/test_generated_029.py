"""Generated filler test 029 for the billing package."""

from __future__ import annotations

from billing.generated.generated_029 import build_billing_payload_029


def test_generated_payload_029() -> None:
    payload = build_billing_payload_029("seed")
    assert payload["identifier"].startswith("seed-")
