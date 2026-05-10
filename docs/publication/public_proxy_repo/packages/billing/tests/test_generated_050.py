"""Generated filler test 050 for the billing package."""

from __future__ import annotations

from billing.generated.generated_050 import build_billing_payload_050


def test_generated_payload_050() -> None:
    payload = build_billing_payload_050("seed")
    assert payload["identifier"].startswith("seed-")
