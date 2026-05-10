"""Generated filler test 034 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_034 import build_notifications_payload_034


def test_generated_payload_034() -> None:
    payload = build_notifications_payload_034("seed")
    assert payload["identifier"].startswith("seed-")
