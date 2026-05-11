"""Generated filler test 004 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_004 import build_experiments_payload_004


def test_generated_payload_004() -> None:
    payload = build_experiments_payload_004("seed")
    assert payload["identifier"].startswith("seed-")
