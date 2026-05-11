"""Generated filler test 035 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_035 import build_experiments_payload_035


def test_generated_payload_035() -> None:
    payload = build_experiments_payload_035("seed")
    assert payload["identifier"].startswith("seed-")
