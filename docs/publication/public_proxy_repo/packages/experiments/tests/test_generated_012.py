"""Generated filler test 012 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_012 import build_experiments_payload_012


def test_generated_payload_012() -> None:
    payload = build_experiments_payload_012("seed")
    assert payload["identifier"].startswith("seed-")
