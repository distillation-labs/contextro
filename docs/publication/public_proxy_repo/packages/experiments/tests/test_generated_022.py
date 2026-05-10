"""Generated filler test 022 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_022 import build_experiments_payload_022


def test_generated_payload_022() -> None:
    payload = build_experiments_payload_022("seed")
    assert payload["identifier"].startswith("seed-")
