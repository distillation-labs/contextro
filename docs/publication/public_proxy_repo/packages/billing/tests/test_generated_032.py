"""Generated filler test 032 for the billing package."""

from __future__ import annotations

from billing.generated.generated_032 import build_billing_payload_032


def test_generated_payload_032() -> None:
    payload = build_billing_payload_032("seed")
    assert payload["identifier"].startswith("seed-")
