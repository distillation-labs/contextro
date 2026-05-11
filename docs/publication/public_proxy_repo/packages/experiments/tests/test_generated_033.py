"""Generated filler test 033 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_033 import build_experiments_payload_033


def test_generated_payload_033() -> None:
    payload = build_experiments_payload_033("seed")
    assert payload["identifier"].startswith("seed-")
