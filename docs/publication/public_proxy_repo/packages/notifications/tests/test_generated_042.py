"""Generated filler test 042 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_042 import build_notifications_payload_042


def test_generated_payload_042() -> None:
    payload = build_notifications_payload_042("seed")
    assert payload["identifier"].startswith("seed-")
