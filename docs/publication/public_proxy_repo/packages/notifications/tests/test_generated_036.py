"""Generated filler test 036 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_036 import build_notifications_payload_036


def test_generated_payload_036() -> None:
    payload = build_notifications_payload_036("seed")
    assert payload["identifier"].startswith("seed-")
