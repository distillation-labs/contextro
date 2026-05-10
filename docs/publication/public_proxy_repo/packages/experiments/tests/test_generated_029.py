"""Generated filler test 029 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_029 import build_experiments_payload_029


def test_generated_payload_029() -> None:
    payload = build_experiments_payload_029("seed")
    assert payload["identifier"].startswith("seed-")
