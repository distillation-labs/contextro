"""Generated filler test 020 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_020 import build_notifications_payload_020


def test_generated_payload_020() -> None:
    payload = build_notifications_payload_020("seed")
    assert payload["identifier"].startswith("seed-")
