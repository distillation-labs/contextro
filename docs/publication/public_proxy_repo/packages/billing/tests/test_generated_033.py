"""Generated filler test 033 for the billing package."""

from __future__ import annotations

from billing.generated.generated_033 import build_billing_payload_033


def test_generated_payload_033() -> None:
    payload = build_billing_payload_033("seed")
    assert payload["identifier"].startswith("seed-")
