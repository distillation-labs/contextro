"""Generated filler test 043 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_043 import build_notifications_payload_043


def test_generated_payload_043() -> None:
    payload = build_notifications_payload_043("seed")
    assert payload["identifier"].startswith("seed-")
