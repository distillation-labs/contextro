"""Worker job for invoice reminder digesting."""

from __future__ import annotations

from billing.invoice_reminders import queue_invoice_reminder_job
from notifications.preferences import resolve_notification_channels


def run_invoice_digest(
    account_name: str,
    invoice_total: int,
    preferences: dict[str, object],
) -> dict[str, object]:
    """Run the digest job that batches invoice reminders."""
    reminder = queue_invoice_reminder_job(account_name, invoice_total)
    channels = resolve_notification_channels(preferences)
    reminder["channels"] = channels
    return reminder
