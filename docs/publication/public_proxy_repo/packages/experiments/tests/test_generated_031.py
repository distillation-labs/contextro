"""Generated filler test 031 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_031 import build_experiments_payload_031


def test_generated_payload_031() -> None:
    payload = build_experiments_payload_031("seed")
    assert payload["identifier"].startswith("seed-")
