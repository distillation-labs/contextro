"""Generated filler test 034 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_034 import build_experiments_payload_034


def test_generated_payload_034() -> None:
    payload = build_experiments_payload_034("seed")
    assert payload["identifier"].startswith("seed-")
