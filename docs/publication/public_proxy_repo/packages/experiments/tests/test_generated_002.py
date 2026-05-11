"""Generated filler test 002 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_002 import build_experiments_payload_002


def test_generated_payload_002() -> None:
    payload = build_experiments_payload_002("seed")
    assert payload["identifier"].startswith("seed-")
