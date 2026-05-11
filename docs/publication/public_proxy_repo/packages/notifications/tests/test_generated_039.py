"""Generated filler test 039 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_039 import build_notifications_payload_039


def test_generated_payload_039() -> None:
    payload = build_notifications_payload_039("seed")
    assert payload["identifier"].startswith("seed-")
