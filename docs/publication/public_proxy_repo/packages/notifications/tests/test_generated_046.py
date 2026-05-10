"""Generated filler test 046 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_046 import build_notifications_payload_046


def test_generated_payload_046() -> None:
    payload = build_notifications_payload_046("seed")
    assert payload["identifier"].startswith("seed-")
