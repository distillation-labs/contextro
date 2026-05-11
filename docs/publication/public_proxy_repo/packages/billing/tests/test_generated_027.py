"""Generated filler test 027 for the billing package."""

from __future__ import annotations

from billing.generated.generated_027 import build_billing_payload_027


def test_generated_payload_027() -> None:
    payload = build_billing_payload_027("seed")
    assert payload["identifier"].startswith("seed-")
