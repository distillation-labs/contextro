"""Generated filler test 013 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_013 import build_notifications_payload_013


def test_generated_payload_013() -> None:
    payload = build_notifications_payload_013("seed")
    assert payload["identifier"].startswith("seed-")
