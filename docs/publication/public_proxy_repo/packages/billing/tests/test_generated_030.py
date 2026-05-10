"""Generated filler test 030 for the billing package."""

from __future__ import annotations

from billing.generated.generated_030 import build_billing_payload_030


def test_generated_payload_030() -> None:
    payload = build_billing_payload_030("seed")
    assert payload["identifier"].startswith("seed-")
