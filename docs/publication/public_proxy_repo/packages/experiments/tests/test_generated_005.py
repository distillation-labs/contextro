"""Generated filler test 005 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_005 import build_experiments_payload_005


def test_generated_payload_005() -> None:
    payload = build_experiments_payload_005("seed")
    assert payload["identifier"].startswith("seed-")
