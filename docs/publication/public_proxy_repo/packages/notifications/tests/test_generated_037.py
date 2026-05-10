"""Generated filler test 037 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_037 import build_notifications_payload_037


def test_generated_payload_037() -> None:
    payload = build_notifications_payload_037("seed")
    assert payload["identifier"].startswith("seed-")
