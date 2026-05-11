"""Generated filler test 044 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_044 import build_experiments_payload_044


def test_generated_payload_044() -> None:
    payload = build_experiments_payload_044("seed")
    assert payload["identifier"].startswith("seed-")
