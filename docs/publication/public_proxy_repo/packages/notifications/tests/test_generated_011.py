"""Generated filler test 011 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_011 import build_notifications_payload_011


def test_generated_payload_011() -> None:
    payload = build_notifications_payload_011("seed")
    assert payload["identifier"].startswith("seed-")
