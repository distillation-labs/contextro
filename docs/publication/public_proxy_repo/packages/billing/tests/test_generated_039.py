"""Generated filler test 039 for the billing package."""

from __future__ import annotations

from billing.generated.generated_039 import build_billing_payload_039


def test_generated_payload_039() -> None:
    payload = build_billing_payload_039("seed")
    assert payload["identifier"].startswith("seed-")
