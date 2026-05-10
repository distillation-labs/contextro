"""Generated filler test 011 for the billing package."""

from __future__ import annotations

from billing.generated.generated_011 import build_billing_payload_011


def test_generated_payload_011() -> None:
    payload = build_billing_payload_011("seed")
    assert payload["identifier"].startswith("seed-")
