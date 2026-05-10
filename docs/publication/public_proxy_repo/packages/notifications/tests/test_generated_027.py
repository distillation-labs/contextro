"""Generated filler test 027 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_027 import build_notifications_payload_027


def test_generated_payload_027() -> None:
    payload = build_notifications_payload_027("seed")
    assert payload["identifier"].startswith("seed-")
