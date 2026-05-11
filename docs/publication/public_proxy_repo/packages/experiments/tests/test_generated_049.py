"""Generated filler test 049 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_049 import build_experiments_payload_049


def test_generated_payload_049() -> None:
    payload = build_experiments_payload_049("seed")
    assert payload["identifier"].startswith("seed-")
