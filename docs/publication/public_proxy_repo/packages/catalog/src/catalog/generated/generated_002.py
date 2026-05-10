"""Generated filler module 002 for the catalog package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class CatalogGenerated002:
    identifier: str
    enabled: bool = True


def build_catalog_payload_002(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated catalog data."""
    record = CatalogGenerated002(identifier=f"{seed}-002")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "catalog"}
