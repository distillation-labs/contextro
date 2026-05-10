"""Generated filler test 017 for the billing package."""

from __future__ import annotations

from billing.generated.generated_017 import build_billing_payload_017


def test_generated_payload_017() -> None:
    payload = build_billing_payload_017("seed")
    assert payload["identifier"].startswith("seed-")
