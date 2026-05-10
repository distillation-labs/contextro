"""Generated filler test 032 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_032 import build_experiments_payload_032


def test_generated_payload_032() -> None:
    payload = build_experiments_payload_032("seed")
    assert payload["identifier"].startswith("seed-")
