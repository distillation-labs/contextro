"""Generated filler test 027 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_027 import build_experiments_payload_027


def test_generated_payload_027() -> None:
    payload = build_experiments_payload_027("seed")
    assert payload["identifier"].startswith("seed-")
