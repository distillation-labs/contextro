"""Generated filler test 019 for the billing package."""

from __future__ import annotations

from billing.generated.generated_019 import build_billing_payload_019


def test_generated_payload_019() -> None:
    payload = build_billing_payload_019("seed")
    assert payload["identifier"].startswith("seed-")
