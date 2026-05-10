"""Generated filler test 006 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_006 import build_notifications_payload_006


def test_generated_payload_006() -> None:
    payload = build_notifications_payload_006("seed")
    assert payload["identifier"].startswith("seed-")
