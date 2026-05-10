"""Generated filler test 048 for the billing package."""

from __future__ import annotations

from billing.generated.generated_048 import build_billing_payload_048


def test_generated_payload_048() -> None:
    payload = build_billing_payload_048("seed")
    assert payload["identifier"].startswith("seed-")
