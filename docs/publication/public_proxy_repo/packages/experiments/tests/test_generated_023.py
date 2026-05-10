"""Generated filler test 023 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_023 import build_experiments_payload_023


def test_generated_payload_023() -> None:
    payload = build_experiments_payload_023("seed")
    assert payload["identifier"].startswith("seed-")
