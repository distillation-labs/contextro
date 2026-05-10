"""Shared immutable flag payloads."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class FlagSnapshot:
    actor_id: str
    flag_name: str
    enabled: bool
