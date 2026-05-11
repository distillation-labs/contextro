"""Generated filler test 019 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_019 import build_experiments_payload_019


def test_generated_payload_019() -> None:
    payload = build_experiments_payload_019("seed")
    assert payload["identifier"].startswith("seed-")
