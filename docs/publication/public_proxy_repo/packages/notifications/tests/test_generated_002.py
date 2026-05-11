"""Generated filler test 002 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_002 import build_notifications_payload_002


def test_generated_payload_002() -> None:
    payload = build_notifications_payload_002("seed")
    assert payload["identifier"].startswith("seed-")
