"""Generated filler test 012 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_012 import build_notifications_payload_012


def test_generated_payload_012() -> None:
    payload = build_notifications_payload_012("seed")
    assert payload["identifier"].startswith("seed-")
