"""Generated filler test 020 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_020 import build_experiments_payload_020


def test_generated_payload_020() -> None:
    payload = build_experiments_payload_020("seed")
    assert payload["identifier"].startswith("seed-")
