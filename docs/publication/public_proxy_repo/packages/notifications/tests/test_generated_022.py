"""Generated filler test 022 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_022 import build_notifications_payload_022


def test_generated_payload_022() -> None:
    payload = build_notifications_payload_022("seed")
    assert payload["identifier"].startswith("seed-")
