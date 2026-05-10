"""Generated filler test 031 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_031 import build_notifications_payload_031


def test_generated_payload_031() -> None:
    payload = build_notifications_payload_031("seed")
    assert payload["identifier"].startswith("seed-")
