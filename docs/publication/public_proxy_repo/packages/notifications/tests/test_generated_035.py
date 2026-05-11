"""Generated filler test 035 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_035 import build_notifications_payload_035


def test_generated_payload_035() -> None:
    payload = build_notifications_payload_035("seed")
    assert payload["identifier"].startswith("seed-")
