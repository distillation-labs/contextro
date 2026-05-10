"""Immutable partner snapshot records."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class PartnerSnapshot:
    alias: str
    account_manager: str
    billing_status: str
