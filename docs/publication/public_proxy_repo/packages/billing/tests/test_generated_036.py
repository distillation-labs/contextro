"""Generated filler test 036 for the billing package."""

from __future__ import annotations

from billing.generated.generated_036 import build_billing_payload_036


def test_generated_payload_036() -> None:
    payload = build_billing_payload_036("seed")
    assert payload["identifier"].startswith("seed-")
