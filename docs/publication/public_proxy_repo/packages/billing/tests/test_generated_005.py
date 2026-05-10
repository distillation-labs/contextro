"""Generated filler test 005 for the billing package."""

from __future__ import annotations

from billing.generated.generated_005 import build_billing_payload_005


def test_generated_payload_005() -> None:
    payload = build_billing_payload_005("seed")
    assert payload["identifier"].startswith("seed-")
