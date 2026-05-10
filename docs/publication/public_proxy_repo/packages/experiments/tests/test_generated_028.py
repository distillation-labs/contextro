"""Generated filler test 028 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_028 import build_experiments_payload_028


def test_generated_payload_028() -> None:
    payload = build_experiments_payload_028("seed")
    assert payload["identifier"].startswith("seed-")
