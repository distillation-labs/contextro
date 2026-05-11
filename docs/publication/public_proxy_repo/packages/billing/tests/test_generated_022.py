"""Generated filler test 022 for the billing package."""

from __future__ import annotations

from billing.generated.generated_022 import build_billing_payload_022


def test_generated_payload_022() -> None:
    payload = build_billing_payload_022("seed")
    assert payload["identifier"].startswith("seed-")
