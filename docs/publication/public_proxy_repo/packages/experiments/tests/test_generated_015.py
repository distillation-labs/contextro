"""Generated filler test 015 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_015 import build_experiments_payload_015


def test_generated_payload_015() -> None:
    payload = build_experiments_payload_015("seed")
    assert payload["identifier"].startswith("seed-")
