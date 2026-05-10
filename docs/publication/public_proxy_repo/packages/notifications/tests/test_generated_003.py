"""Generated filler test 003 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_003 import build_notifications_payload_003


def test_generated_payload_003() -> None:
    payload = build_notifications_payload_003("seed")
    assert payload["identifier"].startswith("seed-")
