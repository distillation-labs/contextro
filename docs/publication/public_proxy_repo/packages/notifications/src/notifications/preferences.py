"""Notification preference resolution for partner operators."""

from __future__ import annotations

DEFAULT_CHANNELS = ("email",)


def resolve_notification_channels(preference_blob: dict[str, object]) -> tuple[str, ...]:
    """Choose delivery channels from user notification preferences."""
    channels = tuple(preference_blob.get("channels", DEFAULT_CHANNELS))
    return channels or DEFAULT_CHANNELS
