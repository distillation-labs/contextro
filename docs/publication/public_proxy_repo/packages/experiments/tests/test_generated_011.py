"""Generated filler test 011 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_011 import build_experiments_payload_011


def test_generated_payload_011() -> None:
    payload = build_experiments_payload_011("seed")
    assert payload["identifier"].startswith("seed-")
