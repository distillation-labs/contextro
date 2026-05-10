"""Generated filler test 046 for the billing package."""

from __future__ import annotations

from billing.generated.generated_046 import build_billing_payload_046


def test_generated_payload_046() -> None:
    payload = build_billing_payload_046("seed")
    assert payload["identifier"].startswith("seed-")
