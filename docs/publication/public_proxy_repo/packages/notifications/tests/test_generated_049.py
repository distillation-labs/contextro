"""Generated filler test 049 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_049 import build_notifications_payload_049


def test_generated_payload_049() -> None:
    payload = build_notifications_payload_049("seed")
    assert payload["identifier"].startswith("seed-")
