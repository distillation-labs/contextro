"""Generated filler test 021 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_021 import build_experiments_payload_021


def test_generated_payload_021() -> None:
    payload = build_experiments_payload_021("seed")
    assert payload["identifier"].startswith("seed-")
