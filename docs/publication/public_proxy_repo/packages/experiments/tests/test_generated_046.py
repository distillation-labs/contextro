"""Generated filler test 046 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_046 import build_experiments_payload_046


def test_generated_payload_046() -> None:
    payload = build_experiments_payload_046("seed")
    assert payload["identifier"].startswith("seed-")
