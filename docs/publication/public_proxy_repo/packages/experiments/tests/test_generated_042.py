"""Generated filler test 042 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_042 import build_experiments_payload_042


def test_generated_payload_042() -> None:
    payload = build_experiments_payload_042("seed")
    assert payload["identifier"].startswith("seed-")
