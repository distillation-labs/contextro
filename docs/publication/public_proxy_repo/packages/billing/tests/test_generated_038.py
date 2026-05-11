"""Generated filler test 038 for the billing package."""

from __future__ import annotations

from billing.generated.generated_038 import build_billing_payload_038


def test_generated_payload_038() -> None:
    payload = build_billing_payload_038("seed")
    assert payload["identifier"].startswith("seed-")
