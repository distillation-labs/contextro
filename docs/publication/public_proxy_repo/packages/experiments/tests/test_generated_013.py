"""Generated filler test 013 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_013 import build_experiments_payload_013


def test_generated_payload_013() -> None:
    payload = build_experiments_payload_013("seed")
    assert payload["identifier"].startswith("seed-")
