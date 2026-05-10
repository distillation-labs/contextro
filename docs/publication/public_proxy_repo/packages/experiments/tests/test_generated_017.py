"""Generated filler test 017 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_017 import build_experiments_payload_017


def test_generated_payload_017() -> None:
    payload = build_experiments_payload_017("seed")
    assert payload["identifier"].startswith("seed-")
