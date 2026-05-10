"""Generated filler test 009 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_009 import build_experiments_payload_009


def test_generated_payload_009() -> None:
    payload = build_experiments_payload_009("seed")
    assert payload["identifier"].startswith("seed-")
