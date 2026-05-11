"""Generated filler test 024 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_024 import build_notifications_payload_024


def test_generated_payload_024() -> None:
    payload = build_notifications_payload_024("seed")
    assert payload["identifier"].startswith("seed-")
