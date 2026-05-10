"""Generated filler test 020 for the billing package."""

from __future__ import annotations

from billing.generated.generated_020 import build_billing_payload_020


def test_generated_payload_020() -> None:
    payload = build_billing_payload_020("seed")
    assert payload["identifier"].startswith("seed-")
