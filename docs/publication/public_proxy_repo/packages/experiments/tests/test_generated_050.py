"""Generated filler test 050 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_050 import build_experiments_payload_050


def test_generated_payload_050() -> None:
    payload = build_experiments_payload_050("seed")
    assert payload["identifier"].startswith("seed-")
