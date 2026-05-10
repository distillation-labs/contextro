"""Generated filler test 050 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_050 import build_notifications_payload_050


def test_generated_payload_050() -> None:
    payload = build_notifications_payload_050("seed")
    assert payload["identifier"].startswith("seed-")
