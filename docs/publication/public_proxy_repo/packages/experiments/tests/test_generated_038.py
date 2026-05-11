"""Generated filler test 038 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_038 import build_experiments_payload_038


def test_generated_payload_038() -> None:
    payload = build_experiments_payload_038("seed")
    assert payload["identifier"].startswith("seed-")
