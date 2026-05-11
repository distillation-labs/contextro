"""Generated filler test 001 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_001 import build_notifications_payload_001


def test_generated_payload_001() -> None:
    payload = build_notifications_payload_001("seed")
    assert payload["identifier"].startswith("seed-")
