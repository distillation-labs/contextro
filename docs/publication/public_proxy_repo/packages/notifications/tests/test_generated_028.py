"""Generated filler test 028 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_028 import build_notifications_payload_028


def test_generated_payload_028() -> None:
    payload = build_notifications_payload_028("seed")
    assert payload["identifier"].startswith("seed-")
