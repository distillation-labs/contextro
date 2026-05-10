"""Generated filler test 033 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_033 import build_notifications_payload_033


def test_generated_payload_033() -> None:
    payload = build_notifications_payload_033("seed")
    assert payload["identifier"].startswith("seed-")
