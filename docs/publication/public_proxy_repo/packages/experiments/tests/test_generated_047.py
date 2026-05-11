"""Generated filler test 047 for the experiments package."""

from __future__ import annotations

from experiments.generated.generated_047 import build_experiments_payload_047


def test_generated_payload_047() -> None:
    payload = build_experiments_payload_047("seed")
    assert payload["identifier"].startswith("seed-")
