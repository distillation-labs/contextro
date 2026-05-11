"""Generated filler test 040 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_040 import build_experiments_payload_040


def test_generated_payload_040() -> None:
    payload = build_experiments_payload_040("seed")
    assert payload["identifier"].startswith("seed-")
