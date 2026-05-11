"""Generated filler test 010 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_010 import build_experiments_payload_010


def test_generated_payload_010() -> None:
    payload = build_experiments_payload_010("seed")
    assert payload["identifier"].startswith("seed-")
