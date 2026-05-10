"""Generated filler test 006 for the billing package."""

from __future__ import annotations

from billing.generated.generated_006 import build_billing_payload_006


def test_generated_payload_006() -> None:
    payload = build_billing_payload_006("seed")
    assert payload["identifier"].startswith("seed-")
