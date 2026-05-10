"""Generated filler test 006 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_006 import build_experiments_payload_006


def test_generated_payload_006() -> None:
    payload = build_experiments_payload_006("seed")
    assert payload["identifier"].startswith("seed-")
