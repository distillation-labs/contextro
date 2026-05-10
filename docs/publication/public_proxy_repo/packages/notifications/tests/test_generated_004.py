"""Generated filler test 004 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_004 import build_notifications_payload_004


def test_generated_payload_004() -> None:
    payload = build_notifications_payload_004("seed")
    assert payload["identifier"].startswith("seed-")
