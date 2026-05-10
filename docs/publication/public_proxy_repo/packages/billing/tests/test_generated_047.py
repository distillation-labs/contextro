"""Generated filler test 047 for the billing package."""

from __future__ import annotations

from billing.generated.generated_047 import build_billing_payload_047


def test_generated_payload_047() -> None:
    payload = build_billing_payload_047("seed")
    assert payload["identifier"].startswith("seed-")
