"""Generated filler test 043 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_043 import build_experiments_payload_043


def test_generated_payload_043() -> None:
    payload = build_experiments_payload_043("seed")
    assert payload["identifier"].startswith("seed-")
