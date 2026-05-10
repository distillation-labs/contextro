"""Generated filler test 026 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_026 import build_experiments_payload_026


def test_generated_payload_026() -> None:
    payload = build_experiments_payload_026("seed")
    assert payload["identifier"].startswith("seed-")
