"""Generated filler test 030 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_030 import build_experiments_payload_030


def test_generated_payload_030() -> None:
    payload = build_experiments_payload_030("seed")
    assert payload["identifier"].startswith("seed-")
