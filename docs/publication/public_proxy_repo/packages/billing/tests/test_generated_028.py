"""Generated filler test 028 for the billing package."""

from __future__ import annotations

from billing.generated.generated_028 import build_billing_payload_028


def test_generated_payload_028() -> None:
    payload = build_billing_payload_028("seed")
    assert payload["identifier"].startswith("seed-")
