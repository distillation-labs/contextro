"""Generated filler test 048 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_048 import build_experiments_payload_048


def test_generated_payload_048() -> None:
    payload = build_experiments_payload_048("seed")
    assert payload["identifier"].startswith("seed-")
