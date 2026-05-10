"""Generated filler test 025 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_025 import build_experiments_payload_025


def test_generated_payload_025() -> None:
    payload = build_experiments_payload_025("seed")
    assert payload["identifier"].startswith("seed-")
