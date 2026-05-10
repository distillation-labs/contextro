"""Generated filler test 034 for the billing package."""

from __future__ import annotations

from billing.generated.generated_034 import build_billing_payload_034


def test_generated_payload_034() -> None:
    payload = build_billing_payload_034("seed")
    assert payload["identifier"].startswith("seed-")
