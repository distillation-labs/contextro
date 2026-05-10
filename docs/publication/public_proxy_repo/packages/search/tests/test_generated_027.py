"""Generated filler test 027 for the search package."""

from __future__ import annotations

from search.generated.generated_027 import build_search_payload_027


def test_generated_payload_027() -> None:
    payload = build_search_payload_027("seed")
    assert payload["identifier"].startswith("seed-")
