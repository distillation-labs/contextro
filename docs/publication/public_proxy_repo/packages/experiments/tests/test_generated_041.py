"""Generated filler test 041 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_041 import build_experiments_payload_041


def test_generated_payload_041() -> None:
    payload = build_experiments_payload_041("seed")
    assert payload["identifier"].startswith("seed-")
