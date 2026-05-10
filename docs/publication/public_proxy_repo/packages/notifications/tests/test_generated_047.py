"""Generated filler test 047 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_047 import build_notifications_payload_047


def test_generated_payload_047() -> None:
    payload = build_notifications_payload_047("seed")
    assert payload["identifier"].startswith("seed-")
