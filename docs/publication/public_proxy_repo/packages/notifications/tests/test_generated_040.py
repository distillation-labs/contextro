"""Generated filler test 040 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_040 import build_notifications_payload_040


def test_generated_payload_040() -> None:
    payload = build_notifications_payload_040("seed")
    assert payload["identifier"].startswith("seed-")
