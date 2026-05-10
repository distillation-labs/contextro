"""Generated filler test 024 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_024 import build_experiments_payload_024


def test_generated_payload_024() -> None:
    payload = build_experiments_payload_024("seed")
    assert payload["identifier"].startswith("seed-")
