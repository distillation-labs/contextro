"""Generated filler test 032 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_032 import build_notifications_payload_032


def test_generated_payload_032() -> None:
    payload = build_notifications_payload_032("seed")
    assert payload["identifier"].startswith("seed-")
