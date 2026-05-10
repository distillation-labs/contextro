"""Generated filler test 001 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_001 import build_experiments_payload_001


def test_generated_payload_001() -> None:
    payload = build_experiments_payload_001("seed")
    assert payload["identifier"].startswith("seed-")
