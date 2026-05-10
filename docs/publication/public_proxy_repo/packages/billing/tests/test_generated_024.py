"""Generated filler test 024 for the billing package."""

from __future__ import annotations

from billing.generated.generated_024 import build_billing_payload_024


def test_generated_payload_024() -> None:
    payload = build_billing_payload_024("seed")
    assert payload["identifier"].startswith("seed-")
