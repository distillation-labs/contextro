"""Generated filler test 040 for the billing package."""

from __future__ import annotations

from billing.generated.generated_040 import build_billing_payload_040


def test_generated_payload_040() -> None:
    payload = build_billing_payload_040("seed")
    assert payload["identifier"].startswith("seed-")
