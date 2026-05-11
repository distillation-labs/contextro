"""Generated filler test 037 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_037 import build_experiments_payload_037


def test_generated_payload_037() -> None:
    payload = build_experiments_payload_037("seed")
    assert payload["identifier"].startswith("seed-")
