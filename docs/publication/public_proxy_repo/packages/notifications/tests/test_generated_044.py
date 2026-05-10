"""Generated filler test 044 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_044 import build_notifications_payload_044


def test_generated_payload_044() -> None:
    payload = build_notifications_payload_044("seed")
    assert payload["identifier"].startswith("seed-")
