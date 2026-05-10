"""Generated filler test 018 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_018 import build_experiments_payload_018


def test_generated_payload_018() -> None:
    payload = build_experiments_payload_018("seed")
    assert payload["identifier"].startswith("seed-")
