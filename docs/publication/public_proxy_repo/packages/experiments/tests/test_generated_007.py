"""Generated filler test 007 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_007 import build_experiments_payload_007


def test_generated_payload_007() -> None:
    payload = build_experiments_payload_007("seed")
    assert payload["identifier"].startswith("seed-")
