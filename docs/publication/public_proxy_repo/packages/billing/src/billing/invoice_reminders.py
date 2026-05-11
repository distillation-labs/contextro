"""Reminder creation for enterprise invoice follow-up."""

from __future__ import annotations

from analytics.reporter import emit_partner_metric
from notifications.digest_scheduler import schedule_digest_delivery


def build_invoice_reminder_message(account_name: str, invoice_total: int) -> str:
    """Build the reminder body sent when enterprise invoices are overdue."""
    return f"Invoice reminder for {account_name}: total={invoice_total}"


def queue_invoice_reminder_job(account_name: str, invoice_total: int) -> dict[str, object]:
    """Queue a reminder job and schedule digest delivery for the billing team."""
    body = build_invoice_reminder_message(account_name, invoice_total)
    schedule = schedule_digest_delivery(account_name, delay_minutes=45)
    emit_partner_metric("billing.reminder.queued", account_name)
    return {"body": body, "schedule": schedule}
