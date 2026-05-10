"""Generated filler test 016 for the billing package."""

from __future__ import annotations

from billing.generated.generated_016 import build_billing_payload_016


def test_generated_payload_016() -> None:
    payload = build_billing_payload_016("seed")
    assert payload["identifier"].startswith("seed-")
