"""Generated filler test 045 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_045 import build_notifications_payload_045


def test_generated_payload_045() -> None:
    payload = build_notifications_payload_045("seed")
    assert payload["identifier"].startswith("seed-")
