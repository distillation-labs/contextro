"""Indexing package exports."""

from __future__ import annotations

from importlib import import_module

__all__ = ["pipeline"]


def __getattr__(name: str):
    """Lazily expose submodules for patching and test helpers."""
    if name == "pipeline":
        return import_module(".pipeline", __name__)
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
