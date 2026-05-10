"""Generated filler test 043 for the billing package."""

from __future__ import annotations

from billing.generated.generated_043 import build_billing_payload_043


def test_generated_payload_043() -> None:
    payload = build_billing_payload_043("seed")
    assert payload["identifier"].startswith("seed-")
