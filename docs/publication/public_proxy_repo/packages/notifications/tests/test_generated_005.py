"""Generated filler test 005 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_005 import build_notifications_payload_005


def test_generated_payload_005() -> None:
    payload = build_notifications_payload_005("seed")
    assert payload["identifier"].startswith("seed-")
