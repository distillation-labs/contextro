"""Generated filler test 019 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_019 import build_notifications_payload_019


def test_generated_payload_019() -> None:
    payload = build_notifications_payload_019("seed")
    assert payload["identifier"].startswith("seed-")
