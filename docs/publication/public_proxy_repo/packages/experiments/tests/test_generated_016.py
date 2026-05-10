"""Generated filler test 016 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_016 import build_experiments_payload_016


def test_generated_payload_016() -> None:
    payload = build_experiments_payload_016("seed")
    assert payload["identifier"].startswith("seed-")
