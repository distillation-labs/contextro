"""Generated filler test 036 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_036 import build_experiments_payload_036


def test_generated_payload_036() -> None:
    payload = build_experiments_payload_036("seed")
    assert payload["identifier"].startswith("seed-")
