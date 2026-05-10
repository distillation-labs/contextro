"""Generated filler test 044 for the auth package."""

from __future__ import annotations

from auth.generated.generated_044 import build_auth_payload_044


def test_generated_payload_044() -> None:
    payload = build_auth_payload_044("seed")
    assert payload["identifier"].startswith("seed-")
