"""Generated filler test 014 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_014 import build_experiments_payload_014


def test_generated_payload_014() -> None:
    payload = build_experiments_payload_014("seed")
    assert payload["identifier"].startswith("seed-")
