"""Generated filler test 029 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_029 import build_notifications_payload_029


def test_generated_payload_029() -> None:
    payload = build_notifications_payload_029("seed")
    assert payload["identifier"].startswith("seed-")
