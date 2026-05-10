"""Generated filler test 003 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_003 import build_experiments_payload_003


def test_generated_payload_003() -> None:
    payload = build_experiments_payload_003("seed")
    assert payload["identifier"].startswith("seed-")
