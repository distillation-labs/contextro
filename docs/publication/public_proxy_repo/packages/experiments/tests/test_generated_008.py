"""Generated filler test 008 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_008 import build_experiments_payload_008


def test_generated_payload_008() -> None:
    payload = build_experiments_payload_008("seed")
    assert payload["identifier"].startswith("seed-")
