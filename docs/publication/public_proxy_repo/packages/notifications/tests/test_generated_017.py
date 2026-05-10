"""Generated filler test 017 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_017 import build_notifications_payload_017


def test_generated_payload_017() -> None:
    payload = build_notifications_payload_017("seed")
    assert payload["identifier"].startswith("seed-")
