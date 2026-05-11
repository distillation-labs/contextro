"""Console view models for partner notification preferences."""

from __future__ import annotations

from notifications.preferences import resolve_notification_channels


def build_preferences_view(blob: dict[str, object]) -> dict[str, object]:
    """Build a notification preferences view model for the console."""
    return {"channels": resolve_notification_channels(blob)}
