"""Generated filler test 045 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_045 import build_experiments_payload_045


def test_generated_payload_045() -> None:
    payload = build_experiments_payload_045("seed")
    assert payload["identifier"].startswith("seed-")
