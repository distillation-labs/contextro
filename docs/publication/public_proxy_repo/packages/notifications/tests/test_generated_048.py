"""Generated filler test 048 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_048 import build_notifications_payload_048


def test_generated_payload_048() -> None:
    payload = build_notifications_payload_048("seed")
    assert payload["identifier"].startswith("seed-")
