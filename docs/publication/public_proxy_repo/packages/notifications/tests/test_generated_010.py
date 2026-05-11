"""Generated filler test 010 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_010 import build_notifications_payload_010


def test_generated_payload_010() -> None:
    payload = build_notifications_payload_010("seed")
    assert payload["identifier"].startswith("seed-")
