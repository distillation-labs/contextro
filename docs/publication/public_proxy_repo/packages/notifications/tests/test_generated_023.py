"""Generated filler test 023 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_023 import build_notifications_payload_023


def test_generated_payload_023() -> None:
    payload = build_notifications_payload_023("seed")
    assert payload["identifier"].startswith("seed-")
