"""Generate a public synthetic proxy repository and benchmark task catalogs.

The proxy repository is intentionally deterministic and fully redistributable.
It is sized to exceed 1,000 files so reviewers can rerun discovery benchmarks
on a nontrivial public codebase without access to the private paired-study repo.
"""

# ruff: noqa: E501

from __future__ import annotations

import json
import shutil
import subprocess
from dataclasses import asdict, dataclass
from pathlib import Path
from textwrap import dedent

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PROXY_REPO = ROOT / "docs" / "publication" / "public_proxy_repo"


@dataclass(frozen=True)
class BaselineStep:
    tool: str
    query: str
    path: str = "."
    glob: str = "*.py"
    max_results: int = 25


@dataclass(frozen=True)
class ProxyDiscoveryTask:
    id: str
    category: str
    prompt: str
    mcp_tool: str
    mcp_args: dict[str, object]
    baseline_steps: tuple[BaselineStep, ...]
    expected_any: tuple[str, ...]
    max_reads: int = 4


@dataclass(frozen=True)
class ProxyEditTask:
    id: str
    prompt: str
    discovery_tool: str
    discovery_args: dict[str, object]
    baseline_steps: tuple[BaselineStep, ...]
    target_file: str
    pattern: str
    replacement: str
    expected_contains: tuple[str, ...]
    expected_not_contains: tuple[str, ...] = ()


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text.strip() + "\n", encoding="utf-8")


def _py_init(path: Path, exports: list[str] | None = None) -> None:
    if exports:
        body = "\n".join(f"from .{name} import *" for name in exports)
    else:
        body = ""
    _write(path / "__init__.py", body)


def _curated_files() -> dict[str, str]:
    return {
        "packages/auth/src/auth/session_tokens.py": dedent(
            '''
            """Session token issuance and revocation flows for partner users."""

            from __future__ import annotations

            import hashlib
            from dataclasses import dataclass

            SESSION_GRACE_MINUTES = 20


            @dataclass(frozen=True)
            class SessionToken:
                actor_id: str
                digest: str
                expires_at: int


            class SessionTokenIssuer:
                """Create short-lived session tokens for console actors."""

                def issue(self, actor_id: str, issued_at: int) -> SessionToken:
                    digest = hashlib.sha256(actor_id.encode("utf-8")).hexdigest()
                    return SessionToken(actor_id=actor_id, digest=digest, expires_at=issued_at + 3600)


            def issue_session_token(actor_id: str, issued_at: int) -> SessionToken:
                """Issue a console session token for a partner actor."""
                return SessionTokenIssuer().issue(actor_id, issued_at)


            def revoke_stale_session_token(actor_id: str) -> str:
                """Revoke session state once the grace window has elapsed."""
                return f"revoke:{actor_id}:{SESSION_GRACE_MINUTES}"
            '''
        ),
        "packages/auth/src/auth/audit_bridge.py": dedent(
            '''
            """Bridge authentication flows to the shared audit stream."""

            from __future__ import annotations

            from analytics.reporter import emit_partner_metric


            def emit_auth_audit_event(event_name: str, actor_id: str) -> dict[str, str]:
                """Emit a partner-scoped audit event and companion metric."""
                emit_partner_metric("auth.audit", actor_id)
                return {"event_name": event_name, "actor_id": actor_id}
            '''
        ),
        "packages/billing/src/billing/invoice_reminders.py": dedent(
            '''
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
            '''
        ),
        "packages/notifications/src/notifications/preferences.py": dedent(
            '''
            """Notification preference resolution for partner operators."""

            from __future__ import annotations

            DEFAULT_CHANNELS = ("email",)


            def resolve_notification_channels(preference_blob: dict[str, object]) -> tuple[str, ...]:
                """Choose delivery channels from user notification preferences."""
                channels = tuple(preference_blob.get("channels", DEFAULT_CHANNELS))
                return channels or DEFAULT_CHANNELS
            '''
        ),
        "packages/notifications/src/notifications/digest_scheduler.py": dedent(
            '''
            """Digest scheduling for batched partner alerts."""

            from __future__ import annotations

            from workflows.retry_policies import build_retry_plan

            DEFAULT_DIGEST_DELAY_MINUTES = 45


            def schedule_digest_delivery(
                account_name: str,
                delay_minutes: int = DEFAULT_DIGEST_DELAY_MINUTES,
            ) -> dict[str, object]:
                """Schedule digest delivery for batched partner notifications."""
                return {
                    "account_name": account_name,
                    "delay_minutes": delay_minutes,
                    "retry": build_retry_plan("digest_delivery"),
                }
            '''
        ),
        "packages/partners/src/partners/aliases.py": dedent(
            '''
            """Normalization helpers for partner aliases and lookup keys."""

            from __future__ import annotations


            def normalize_partner_alias(alias: str) -> str:
                """Normalize alias keys before loading partner records."""
                return alias.strip().lower().replace(" ", "-").replace("/", "-")
            '''
        ),
        "packages/partners/src/partners/onboarding.py": dedent(
            '''
            """Context assembly for partner onboarding review workflows."""

            from __future__ import annotations

            from auth.audit_bridge import emit_auth_audit_event
            from catalog.profile_store import load_partner_profile
            from shared.context import build_actor_context

            from partners.aliases import normalize_partner_alias


            def prepare_partner_onboarding_context(
                alias: str, actor_id: str
            ) -> dict[str, object]:
                """Prepare the dashboard context used before partner onboarding review."""
                normalized = normalize_partner_alias(alias)
                actor_context = build_actor_context(actor_id)
                profile = load_partner_profile(normalized)
                emit_auth_audit_event("partner_onboarding.reviewed", actor_id)
                return {
                    "normalized_alias": normalized,
                    "actor_context": actor_context,
                    "profile": profile,
                }
            '''
        ),
        "packages/search/src/search/projections.py": dedent(
            '''
            """Projection maintenance for denormalized partner search rows."""

            from __future__ import annotations

            from shared.serializers import serialize_projection_row

            PROJECTION_BATCH_SIZE = 100


            def refresh_partner_search_projection(
                partner_id: str,
                profile: dict[str, object],
            ) -> dict[str, object]:
                """Refresh denormalized search rows after partner changes."""
                payload = serialize_projection_row(partner_id, profile)
                return {
                    "partner_id": partner_id,
                    "payload": payload,
                    "batch_size": PROJECTION_BATCH_SIZE,
                }
            '''
        ),
        "packages/search/src/search/ranking.py": dedent(
            '''
            """Ranking heuristics for partner search results."""

            from __future__ import annotations


            def score_partner_hits(text_score: float, freshness_score: float, affinity_score: float) -> float:
                """Compute weighted ranking for partner discovery hits."""
                return round((text_score * 0.5) + (freshness_score * 0.2) + (affinity_score * 0.3), 4)
            '''
        ),
        "packages/experiments/src/experiments/rollouts.py": dedent(
            '''
            """Rollout gating for staged partner experiments."""

            from __future__ import annotations

            MIN_SAMPLE_SIZE = 200


            def evaluate_rollout_gate(actor_id: str, sample_size: int, bucket: int) -> bool:
                """Evaluate if a staged rollout gate should open for an actor."""
                if sample_size < MIN_SAMPLE_SIZE:
                    return False
                return bucket % 10 < 3 and actor_id != "blocked-actor"
            '''
        ),
        "packages/analytics/src/analytics/reporter.py": dedent(
            '''
            """Metric fan-out helpers for partner lifecycle workflows."""

            from __future__ import annotations

            PARTNER_METRIC_PREFIX = "partners.lifecycle"


            def emit_partner_metric(metric_name: str, actor_id: str) -> str:
                """Emit a prefixed metric entry for downstream reporting."""
                return f"{PARTNER_METRIC_PREFIX}.{metric_name}:{actor_id}"
            '''
        ),
        "packages/workflows/src/workflows/retry_policies.py": dedent(
            '''
            """Retry-plan helpers for background partner jobs."""

            from __future__ import annotations

            DEFAULT_BACKOFF_FACTOR = 2.0


            def build_retry_plan(job_name: str) -> dict[str, object]:
                """Build a retry plan for a flaky background workflow."""
                return {"job_name": job_name, "attempts": 4, "backoff_factor": DEFAULT_BACKOFF_FACTOR}
            '''
        ),
        "packages/shared/src/shared/context.py": dedent(
            '''
            """Actor context builders used across console and worker flows."""

            from __future__ import annotations


            def build_actor_context(actor_id: str) -> dict[str, object]:
                """Build a normalized actor context for partner-facing workflows."""
                return {"actor_id": actor_id, "actor_scope": "partner"}
            '''
        ),
        "packages/shared/src/shared/serializers.py": dedent(
            '''
            """Serialization helpers for denormalized worker payloads."""

            from __future__ import annotations


            def serialize_projection_row(partner_id: str, profile: dict[str, object]) -> dict[str, object]:
                """Serialize projection rows for downstream fan-out workers."""
                return {"partner_id": partner_id, "profile": profile, "kind": "projection_row"}
            '''
        ),
        "packages/shared/src/shared/flags.py": dedent(
            '''
            """Shared immutable flag payloads."""

            from __future__ import annotations

            from dataclasses import dataclass


            @dataclass(frozen=True)
            class FlagSnapshot:
                actor_id: str
                flag_name: str
                enabled: bool
            '''
        ),
        "packages/catalog/src/catalog/profile_store.py": dedent(
            '''
            """Partner profile access for console and worker tasks."""

            from __future__ import annotations


            def load_partner_profile(alias: str) -> dict[str, object]:
                """Load the latest partner profile for a normalized alias."""
                return {"alias": alias, "tier": "enterprise", "status": "active"}
            '''
        ),
        "packages/catalog/src/catalog/profile_projector.py": dedent(
            '''
            """Projection builder for partner profile cards."""

            from __future__ import annotations

            from shared.serializers import serialize_projection_row


            class PartnerProfileProjector:
                """Project profile rows into dashboard-friendly partner cards."""

                def project(self, alias: str, profile: dict[str, object]) -> dict[str, object]:
                    base_row = serialize_projection_row(alias, profile)
                    return {"alias": alias, "row": base_row, "layout": "partner_card"}
            '''
        ),
        "packages/catalog/src/catalog/snapshots.py": dedent(
            '''
            """Immutable partner snapshot records."""

            from __future__ import annotations

            from dataclasses import dataclass


            @dataclass(frozen=True)
            class PartnerSnapshot:
                alias: str
                account_manager: str
                billing_status: str
            '''
        ),
        "apps/console/src/console/dashboard.py": dedent(
            '''
            """Console dashboard orchestration for partner operators."""

            from __future__ import annotations

            from catalog.profile_projector import PartnerProfileProjector
            from experiments.rollouts import evaluate_rollout_gate
            from partners.onboarding import prepare_partner_onboarding_context


            def build_partner_dashboard(alias: str, actor_id: str) -> dict[str, object]:
                """Build the operator dashboard for a partner profile review."""
                context = prepare_partner_onboarding_context(alias, actor_id)
                projector = PartnerProfileProjector()
                card = projector.project(alias, context["profile"])
                card["show_rollout_banner"] = evaluate_rollout_gate(actor_id, sample_size=320, bucket=2)
                return {"context": context, "card": card}
            '''
        ),
        "apps/console/src/console/preferences_view.py": dedent(
            '''
            """Console view models for partner notification preferences."""

            from __future__ import annotations

            from notifications.preferences import resolve_notification_channels


            def build_preferences_view(blob: dict[str, object]) -> dict[str, object]:
                """Build a notification preferences view model for the console."""
                return {"channels": resolve_notification_channels(blob)}
            '''
        ),
        "apps/worker/src/worker/invoice_digests.py": dedent(
            '''
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
            '''
        ),
        "apps/worker/src/worker/search_projection.py": dedent(
            '''
            """Worker job for partner search projection refreshes."""

            from __future__ import annotations

            from search.projections import refresh_partner_search_projection


            def run_projection_refresh(partner_id: str, profile: dict[str, object]) -> dict[str, object]:
                """Run the background projection refresh after profile changes."""
                return refresh_partner_search_projection(partner_id, profile)
            '''
        ),
        "apps/worker/src/worker/audit_fanout.py": dedent(
            '''
            """Worker fan-out for partner metrics and audit replication."""

            from __future__ import annotations

            from analytics.reporter import emit_partner_metric
            from auth.audit_bridge import emit_auth_audit_event


            def fan_out_partner_audit(actor_id: str) -> list[dict[str, str]]:
                """Fan out audit and metric side effects for onboarding review."""
                metric = emit_partner_metric("partner.audit.replayed", actor_id)
                event = emit_auth_audit_event("partner_onboarding.reviewed", actor_id)
                return [{"metric": metric}, event]
            '''
        ),
    }


def _filler_module(package_name: str, index: int) -> str:
    class_name = f"{package_name.title()}Generated{index:03d}"
    function_name = f"build_{package_name}_payload_{index:03d}"
    return dedent(
        f'''
        """Generated filler module {index:03d} for the {package_name} package."""

        from __future__ import annotations

        from dataclasses import dataclass


        @dataclass(frozen=True)
        class {class_name}:
            identifier: str
            enabled: bool = True


        def {function_name}(seed: str) -> dict[str, object]:
            """Create a deterministic payload for generated {package_name} data."""
            record = {class_name}(identifier=f"{{seed}}-{index:03d}")
            return {{"identifier": record.identifier, "enabled": record.enabled, "package": "{package_name}"}}
        '''
    )


def _filler_test(package_name: str, index: int) -> str:
    function_name = f"build_{package_name}_payload_{index:03d}"
    return dedent(
        f'''
        """Generated filler test {index:03d} for the {package_name} package."""

        from __future__ import annotations

        from {package_name}.generated.generated_{index:03d} import {function_name}


        def test_generated_payload_{index:03d}() -> None:
            payload = {function_name}("seed")
            assert payload["identifier"].startswith("seed-")
        '''
    )


DISCOVERY_TASKS: tuple[ProxyDiscoveryTask, ...] = (
    ProxyDiscoveryTask(
        id="sym_01",
        category="symbol_discovery",
        prompt="Find PartnerProfileProjector.",
        mcp_tool="find_symbol",
        mcp_args={"name": "PartnerProfileProjector"},
        baseline_steps=(BaselineStep("git_grep", "PartnerProfileProjector"),),
        expected_any=("packages/catalog/src/catalog/profile_projector.py",),
    ),
    ProxyDiscoveryTask(
        id="sym_02",
        category="symbol_discovery",
        prompt="Find schedule_digest_delivery.",
        mcp_tool="find_symbol",
        mcp_args={"name": "schedule_digest_delivery"},
        baseline_steps=(BaselineStep("git_grep", "schedule_digest_delivery"),),
        expected_any=("packages/notifications/src/notifications/digest_scheduler.py",),
    ),
    ProxyDiscoveryTask(
        id="sym_03",
        category="symbol_discovery",
        prompt="Find normalize_partner_alias.",
        mcp_tool="find_symbol",
        mcp_args={"name": "normalize_partner_alias"},
        baseline_steps=(BaselineStep("git_grep", "normalize_partner_alias"),),
        expected_any=("packages/partners/src/partners/aliases.py",),
    ),
    ProxyDiscoveryTask(
        id="sym_04",
        category="symbol_discovery",
        prompt="Find refresh_partner_search_projection.",
        mcp_tool="find_symbol",
        mcp_args={"name": "refresh_partner_search_projection"},
        baseline_steps=(BaselineStep("git_grep", "refresh_partner_search_projection"),),
        expected_any=("packages/search/src/search/projections.py",),
    ),
    ProxyDiscoveryTask(
        id="sym_05",
        category="symbol_discovery",
        prompt="Find build_invoice_reminder_message.",
        mcp_tool="find_symbol",
        mcp_args={"name": "build_invoice_reminder_message"},
        baseline_steps=(BaselineStep("git_grep", "build_invoice_reminder_message"),),
        expected_any=("packages/billing/src/billing/invoice_reminders.py",),
    ),
    ProxyDiscoveryTask(
        id="sym_06",
        category="symbol_discovery",
        prompt="Find resolve_notification_channels.",
        mcp_tool="find_symbol",
        mcp_args={"name": "resolve_notification_channels"},
        baseline_steps=(BaselineStep("git_grep", "resolve_notification_channels"),),
        expected_any=("packages/notifications/src/notifications/preferences.py",),
    ),
    ProxyDiscoveryTask(
        id="sym_07",
        category="symbol_discovery",
        prompt="Find evaluate_rollout_gate.",
        mcp_tool="find_symbol",
        mcp_args={"name": "evaluate_rollout_gate"},
        baseline_steps=(BaselineStep("git_grep", "evaluate_rollout_gate"),),
        expected_any=("packages/experiments/src/experiments/rollouts.py",),
    ),
    ProxyDiscoveryTask(
        id="sym_08",
        category="symbol_discovery",
        prompt="Find emit_partner_metric.",
        mcp_tool="find_symbol",
        mcp_args={"name": "emit_partner_metric"},
        baseline_steps=(BaselineStep("git_grep", "emit_partner_metric"),),
        expected_any=("packages/analytics/src/analytics/reporter.py",),
    ),
    ProxyDiscoveryTask(
        id="call_01",
        category="caller_tracing",
        prompt="Who calls refresh_partner_search_projection?",
        mcp_tool="find_callers",
        mcp_args={"symbol_name": "refresh_partner_search_projection"},
        baseline_steps=(BaselineStep("rg", r"refresh_partner_search_projection\("),),
        expected_any=("apps/worker/src/worker/search_projection.py",),
    ),
    ProxyDiscoveryTask(
        id="call_02",
        category="caller_tracing",
        prompt="Who calls schedule_digest_delivery?",
        mcp_tool="find_callers",
        mcp_args={"symbol_name": "schedule_digest_delivery"},
        baseline_steps=(BaselineStep("rg", r"schedule_digest_delivery\("),),
        expected_any=("packages/billing/src/billing/invoice_reminders.py",),
    ),
    ProxyDiscoveryTask(
        id="call_03",
        category="caller_tracing",
        prompt="Who calls normalize_partner_alias?",
        mcp_tool="find_callers",
        mcp_args={"symbol_name": "normalize_partner_alias"},
        baseline_steps=(BaselineStep("rg", r"normalize_partner_alias\("),),
        expected_any=("packages/partners/src/partners/onboarding.py",),
    ),
    ProxyDiscoveryTask(
        id="call_04",
        category="caller_tracing",
        prompt="Who calls emit_partner_metric?",
        mcp_tool="find_callers",
        mcp_args={"symbol_name": "emit_partner_metric"},
        baseline_steps=(BaselineStep("rg", r"emit_partner_metric\("),),
        expected_any=("apps/worker/src/worker/audit_fanout.py",),
    ),
    ProxyDiscoveryTask(
        id="call_05",
        category="caller_tracing",
        prompt="What does prepare_partner_onboarding_context call?",
        mcp_tool="find_callees",
        mcp_args={"symbol_name": "prepare_partner_onboarding_context"},
        baseline_steps=(BaselineStep("git_grep", "prepare_partner_onboarding_context"),),
        expected_any=("build_actor_context", "load_partner_profile"),
    ),
    ProxyDiscoveryTask(
        id="call_06",
        category="caller_tracing",
        prompt="What does queue_invoice_reminder_job call?",
        mcp_tool="find_callees",
        mcp_args={"symbol_name": "queue_invoice_reminder_job"},
        baseline_steps=(BaselineStep("git_grep", "queue_invoice_reminder_job"),),
        expected_any=("schedule_digest_delivery", "build_invoice_reminder_message"),
    ),
    ProxyDiscoveryTask(
        id="search_01",
        category="semantic_search",
        prompt="Find the code that prepares the dashboard context used before partner onboarding review.",
        mcp_tool="search",
        mcp_args={"query": "prepare the dashboard context used before partner onboarding review", "limit": 5},
        baseline_steps=(BaselineStep("rg", r"dashboard context|partner onboarding review|normalized alias"),),
        expected_any=("packages/partners/src/partners/onboarding.py",),
    ),
    ProxyDiscoveryTask(
        id="search_02",
        category="semantic_search",
        prompt="Find the code that builds the reminder body sent when enterprise invoices are overdue.",
        mcp_tool="search",
        mcp_args={"query": "build the reminder body sent when enterprise invoices are overdue", "limit": 5},
        baseline_steps=(BaselineStep("rg", r"reminder body|enterprise invoices|overdue"),),
        expected_any=("packages/billing/src/billing/invoice_reminders.py",),
    ),
    ProxyDiscoveryTask(
        id="search_03",
        category="semantic_search",
        prompt="Find the code that chooses delivery channels from user notification preferences.",
        mcp_tool="search",
        mcp_args={"query": "choose delivery channels from user notification preferences", "limit": 5},
        baseline_steps=(BaselineStep("rg", r"delivery channels|notification preferences|channels"),),
        expected_any=("packages/notifications/src/notifications/preferences.py",),
    ),
    ProxyDiscoveryTask(
        id="search_04",
        category="semantic_search",
        prompt="Find the code that normalizes alias keys before loading partner records.",
        mcp_tool="search",
        mcp_args={"query": "normalize alias keys before loading partner records", "limit": 5},
        baseline_steps=(BaselineStep("rg", r"normalize alias|loading partner records|alias keys"),),
        expected_any=("packages/partners/src/partners/aliases.py",),
    ),
    ProxyDiscoveryTask(
        id="search_05",
        category="semantic_search",
        prompt="Find the code that refreshes denormalized search rows after partner changes.",
        mcp_tool="search",
        mcp_args={"query": "refresh denormalized search rows after partner changes", "limit": 5},
        baseline_steps=(BaselineStep("rg", r"denormalized search rows|projection rows|partner changes"),),
        expected_any=("packages/search/src/search/projections.py",),
    ),
    ProxyDiscoveryTask(
        id="search_06",
        category="semantic_search",
        prompt="Find the code that computes weighted ranking for partner discovery hits.",
        mcp_tool="search",
        mcp_args={"query": "compute weighted ranking for partner discovery hits", "limit": 5},
        baseline_steps=(BaselineStep("rg", r"weighted ranking|partner discovery hits|freshness_score"),),
        expected_any=("packages/search/src/search/ranking.py",),
    ),
    ProxyDiscoveryTask(
        id="search_07",
        category="semantic_search",
        prompt="Find the code that evaluates if a staged rollout gate should open for an actor.",
        mcp_tool="search",
        mcp_args={"query": "evaluate if a staged rollout gate should open for an actor", "limit": 5},
        baseline_steps=(BaselineStep("rg", r"staged rollout gate|sample_size|bucket"),),
        expected_any=("packages/experiments/src/experiments/rollouts.py",),
    ),
    ProxyDiscoveryTask(
        id="search_08",
        category="semantic_search",
        prompt="Find the code that serializes projection rows for downstream fan-out workers.",
        mcp_tool="search",
        mcp_args={"query": "serialize projection rows for downstream fan-out workers", "limit": 5},
        baseline_steps=(BaselineStep("rg", r"projection rows|fan-out workers|serialize"),),
        expected_any=("packages/shared/src/shared/serializers.py",),
    ),
    ProxyDiscoveryTask(
        id="explain_01",
        category="code_understanding",
        prompt="Explain PartnerProfileProjector.",
        mcp_tool="explain",
        mcp_args={"symbol_name": "PartnerProfileProjector"},
        baseline_steps=(BaselineStep("git_grep", "PartnerProfileProjector"),),
        expected_any=("packages/catalog/src/catalog/profile_projector.py",),
    ),
    ProxyDiscoveryTask(
        id="explain_02",
        category="code_understanding",
        prompt="Explain prepare_partner_onboarding_context.",
        mcp_tool="explain",
        mcp_args={"symbol_name": "prepare_partner_onboarding_context"},
        baseline_steps=(BaselineStep("git_grep", "prepare_partner_onboarding_context"),),
        expected_any=("packages/partners/src/partners/onboarding.py",),
    ),
    ProxyDiscoveryTask(
        id="explain_03",
        category="code_understanding",
        prompt="Explain build_retry_plan.",
        mcp_tool="explain",
        mcp_args={"symbol_name": "build_retry_plan"},
        baseline_steps=(BaselineStep("git_grep", "build_retry_plan"),),
        expected_any=("packages/workflows/src/workflows/retry_policies.py",),
    ),
    ProxyDiscoveryTask(
        id="explain_04",
        category="code_understanding",
        prompt="Explain queue_invoice_reminder_job.",
        mcp_tool="explain",
        mcp_args={"symbol_name": "queue_invoice_reminder_job"},
        baseline_steps=(BaselineStep("git_grep", "queue_invoice_reminder_job"),),
        expected_any=("packages/billing/src/billing/invoice_reminders.py",),
    ),
    ProxyDiscoveryTask(
        id="explain_05",
        category="code_understanding",
        prompt="Explain refresh_partner_search_projection.",
        mcp_tool="explain",
        mcp_args={"symbol_name": "refresh_partner_search_projection"},
        baseline_steps=(BaselineStep("git_grep", "refresh_partner_search_projection"),),
        expected_any=("packages/search/src/search/projections.py",),
    ),
    ProxyDiscoveryTask(
        id="explain_06",
        category="code_understanding",
        prompt="Explain emit_auth_audit_event.",
        mcp_tool="explain",
        mcp_args={"symbol_name": "emit_auth_audit_event"},
        baseline_steps=(BaselineStep("git_grep", "emit_auth_audit_event"),),
        expected_any=("packages/auth/src/auth/audit_bridge.py",),
    ),
    ProxyDiscoveryTask(
        id="impact_01",
        category="impact_analysis",
        prompt="What would changing normalize_partner_alias affect?",
        mcp_tool="impact",
        mcp_args={"symbol_name": "normalize_partner_alias", "max_depth": 5},
        baseline_steps=(BaselineStep("rg", r"normalize_partner_alias\("),),
        expected_any=("packages/partners/src/partners/onboarding.py",),
    ),
    ProxyDiscoveryTask(
        id="impact_02",
        category="impact_analysis",
        prompt="What would changing resolve_notification_channels affect?",
        mcp_tool="impact",
        mcp_args={"symbol_name": "resolve_notification_channels", "max_depth": 5},
        baseline_steps=(BaselineStep("rg", r"resolve_notification_channels\("),),
        expected_any=("apps/worker/src/worker/invoice_digests.py", "apps/console/src/console/preferences_view.py"),
    ),
    ProxyDiscoveryTask(
        id="impact_03",
        category="impact_analysis",
        prompt="What would changing emit_partner_metric affect?",
        mcp_tool="impact",
        mcp_args={"symbol_name": "emit_partner_metric", "max_depth": 5},
        baseline_steps=(BaselineStep("rg", r"emit_partner_metric\("),),
        expected_any=("apps/worker/src/worker/audit_fanout.py", "packages/auth/src/auth/audit_bridge.py"),
    ),
    ProxyDiscoveryTask(
        id="impact_04",
        category="impact_analysis",
        prompt="What would changing build_actor_context affect?",
        mcp_tool="impact",
        mcp_args={"symbol_name": "build_actor_context", "max_depth": 5},
        baseline_steps=(BaselineStep("rg", r"build_actor_context\("),),
        expected_any=("packages/partners/src/partners/onboarding.py",),
    ),
    ProxyDiscoveryTask(
        id="git_01",
        category="git_history",
        prompt="Find the commit that adjusted digest delay handling for billing alerts.",
        mcp_tool="commit_search",
        mcp_args={"query": "digest delay handling for billing alerts", "limit": 5},
        baseline_steps=(BaselineStep("git_log", "digest delay handling", path="packages/notifications/src/notifications/digest_scheduler.py"),),
        expected_any=("Adjust digest delay handling for billing alerts",),
        max_reads=0,
    ),
    ProxyDiscoveryTask(
        id="git_02",
        category="git_history",
        prompt="Find the commit that normalized partner alias keys before onboarding lookup.",
        mcp_tool="commit_search",
        mcp_args={"query": "normalized partner alias keys before onboarding lookup", "limit": 5},
        baseline_steps=(
            BaselineStep(
                "git_log",
                "Normalize partner alias keys before onboarding lookup",
                path="packages/partners/src/partners/aliases.py",
            ),
        ),
        expected_any=("Normalize partner alias keys before onboarding lookup",),
        max_reads=0,
    ),
    ProxyDiscoveryTask(
        id="code_01",
        category="code_tool",
        prompt="Use code search_symbols to locate the profile projector symbol.",
        mcp_tool="code",
        mcp_args={"operation": "search_symbols", "symbol_name": "ProfileProjector", "path": "packages/catalog/src", "limit": 5},
        baseline_steps=(BaselineStep("rg", "ProfileProjector", path="packages/catalog/src"),),
        expected_any=("packages/catalog/src/catalog/profile_projector.py",),
    ),
    ProxyDiscoveryTask(
        id="code_02",
        category="code_tool",
        prompt="List top-level symbols in the digest scheduler document.",
        mcp_tool="code",
        mcp_args={"operation": "get_document_symbols", "file_path": "packages/notifications/src/notifications/digest_scheduler.py", "top_level_only": True},
        baseline_steps=(BaselineStep("git_grep", "schedule_digest_delivery", path="packages/notifications/src/notifications"),),
        expected_any=("schedule_digest_delivery",),
        max_reads=1,
    ),
    ProxyDiscoveryTask(
        id="code_03",
        category="code_tool",
        prompt="Map the worker jobs directory to find invoice digest code.",
        mcp_tool="code",
        mcp_args={"operation": "search_codebase_map", "path": "apps/worker/src/worker", "limit": 10},
        baseline_steps=(BaselineStep("git_grep", "run_invoice_digest", path="apps/worker/src/worker"),),
        expected_any=("invoice_digests", "run_invoice_digest"),
    ),
    ProxyDiscoveryTask(
        id="focus_01",
        category="focus",
        prompt="Focus the onboarding context file.",
        mcp_tool="focus",
        mcp_args={"path": "packages/partners/src/partners/onboarding.py", "include_code": True},
        baseline_steps=(BaselineStep("git_grep", "prepare_partner_onboarding_context", path="packages/partners/src/partners"),),
        expected_any=("packages/partners/src/partners/onboarding.py", "normalize_partner_alias"),
        max_reads=1,
    ),
    ProxyDiscoveryTask(
        id="focus_02",
        category="focus",
        prompt="Focus the invoice digest worker file.",
        mcp_tool="focus",
        mcp_args={"path": "apps/worker/src/worker/invoice_digests.py", "include_code": True},
        baseline_steps=(BaselineStep("git_grep", "run_invoice_digest", path="apps/worker/src/worker"),),
        expected_any=("apps/worker/src/worker/invoice_digests.py", "queue_invoice_reminder_job"),
        max_reads=1,
    ),
)


EDIT_TASKS: tuple[ProxyEditTask, ...] = (
    ProxyEditTask(
        id="edit_01",
        prompt="Increase the session grace window constant used by stale-session revocation.",
        discovery_tool="find_symbol",
        discovery_args={"name": "revoke_stale_session_token"},
        baseline_steps=(BaselineStep("git_grep", "SESSION_GRACE_MINUTES"),),
        target_file="packages/auth/src/auth/session_tokens.py",
        pattern="SESSION_GRACE_MINUTES = 20",
        replacement="SESSION_GRACE_MINUTES = 30",
        expected_contains=("SESSION_GRACE_MINUTES = 30",),
        expected_not_contains=("SESSION_GRACE_MINUTES = 20",),
    ),
    ProxyEditTask(
        id="edit_02",
        prompt="Adjust the default digest delay for batched partner notifications.",
        discovery_tool="find_symbol",
        discovery_args={"name": "schedule_digest_delivery"},
        baseline_steps=(BaselineStep("git_grep", "DEFAULT_DIGEST_DELAY_MINUTES"),),
        target_file="packages/notifications/src/notifications/digest_scheduler.py",
        pattern="DEFAULT_DIGEST_DELAY_MINUTES = 45",
        replacement="DEFAULT_DIGEST_DELAY_MINUTES = 60",
        expected_contains=("DEFAULT_DIGEST_DELAY_MINUTES = 60",),
        expected_not_contains=("DEFAULT_DIGEST_DELAY_MINUTES = 45",),
    ),
    ProxyEditTask(
        id="edit_03",
        prompt="Lower the rollout minimum sample size threshold.",
        discovery_tool="find_symbol",
        discovery_args={"name": "evaluate_rollout_gate"},
        baseline_steps=(BaselineStep("git_grep", "MIN_SAMPLE_SIZE"),),
        target_file="packages/experiments/src/experiments/rollouts.py",
        pattern="MIN_SAMPLE_SIZE = 200",
        replacement="MIN_SAMPLE_SIZE = 150",
        expected_contains=("MIN_SAMPLE_SIZE = 150",),
        expected_not_contains=("MIN_SAMPLE_SIZE = 200",),
    ),
    ProxyEditTask(
        id="edit_04",
        prompt="Change the partner metric prefix to the activity namespace.",
        discovery_tool="find_symbol",
        discovery_args={"name": "emit_partner_metric"},
        baseline_steps=(BaselineStep("git_grep", "PARTNER_METRIC_PREFIX"),),
        target_file="packages/analytics/src/analytics/reporter.py",
        pattern='PARTNER_METRIC_PREFIX = "partners.lifecycle"',
        replacement='PARTNER_METRIC_PREFIX = "partners.activity"',
        expected_contains=('PARTNER_METRIC_PREFIX = "partners.activity"',),
        expected_not_contains=('PARTNER_METRIC_PREFIX = "partners.lifecycle"',),
    ),
    ProxyEditTask(
        id="edit_05",
        prompt="Make email and slack the default notification channels.",
        discovery_tool="find_symbol",
        discovery_args={"name": "resolve_notification_channels"},
        baseline_steps=(BaselineStep("git_grep", "DEFAULT_CHANNELS"),),
        target_file="packages/notifications/src/notifications/preferences.py",
        pattern='DEFAULT_CHANNELS = ("email",)',
        replacement='DEFAULT_CHANNELS = ("email", "slack")',
        expected_contains=('DEFAULT_CHANNELS = ("email", "slack")',),
        expected_not_contains=('DEFAULT_CHANNELS = ("email",)',),
    ),
    ProxyEditTask(
        id="edit_06",
        prompt="Increase the projection batch size used by search refreshes.",
        discovery_tool="find_symbol",
        discovery_args={"name": "refresh_partner_search_projection"},
        baseline_steps=(BaselineStep("git_grep", "PROJECTION_BATCH_SIZE"),),
        target_file="packages/search/src/search/projections.py",
        pattern="PROJECTION_BATCH_SIZE = 100",
        replacement="PROJECTION_BATCH_SIZE = 120",
        expected_contains=("PROJECTION_BATCH_SIZE = 120",),
        expected_not_contains=("PROJECTION_BATCH_SIZE = 100",),
    ),
    ProxyEditTask(
        id="edit_07",
        prompt="Reduce the default retry backoff factor for digest delivery jobs.",
        discovery_tool="find_symbol",
        discovery_args={"name": "build_retry_plan"},
        baseline_steps=(BaselineStep("git_grep", "DEFAULT_BACKOFF_FACTOR"),),
        target_file="packages/workflows/src/workflows/retry_policies.py",
        pattern="DEFAULT_BACKOFF_FACTOR = 2.0",
        replacement="DEFAULT_BACKOFF_FACTOR = 1.5",
        expected_contains=("DEFAULT_BACKOFF_FACTOR = 1.5",),
        expected_not_contains=("DEFAULT_BACKOFF_FACTOR = 2.0",),
    ),
    ProxyEditTask(
        id="edit_08",
        prompt="Rename the onboarding review audit event to screened.",
        discovery_tool="search",
        discovery_args={"query": "partner onboarding review audit event", "limit": 5},
        baseline_steps=(BaselineStep("rg", "partner_onboarding.reviewed"),),
        target_file="packages/partners/src/partners/onboarding.py",
        pattern='"partner_onboarding.reviewed"',
        replacement='"partner_onboarding.screened"',
        expected_contains=('"partner_onboarding.screened"',),
        expected_not_contains=('"partner_onboarding.reviewed"',),
    ),
)


def materialize_public_proxy_repo(root: Path = DEFAULT_PROXY_REPO) -> dict[str, object]:
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True, exist_ok=True)

    _write(
        root / "README.md",
        dedent(
            """
            # Contextro Public Proxy Repository

            This synthetic repository is a redistributable benchmark fixture for the
            publication package. It mirrors a multi-package production code layout
            without exposing any private source or identifiers from the original study.
            """
        ),
    )
    _write(
        root / "pyproject.toml",
        dedent(
            """
            [project]
            name = "contextro-public-proxy-repo"
            version = "0.1.0"
            requires-python = ">=3.11"
            """
        ),
    )

    for package in (
        "auth",
        "billing",
        "notifications",
        "partners",
        "search",
        "experiments",
        "analytics",
        "workflows",
        "shared",
        "catalog",
    ):
        _py_init(root / "packages" / package / "src" / package)
        _py_init(root / "packages" / package / "src" / package / "generated")
        _py_init(root / "packages" / package / "tests")

    for app in ("console", "worker"):
        _py_init(root / "apps" / app / "src" / app)
        _py_init(root / "apps" / app / "src" / app / "generated")

    for relative_path, content in _curated_files().items():
        _write(root / relative_path, content)

    filler_packages = (
        "auth",
        "billing",
        "notifications",
        "partners",
        "search",
        "experiments",
        "analytics",
        "workflows",
        "shared",
        "catalog",
    )
    for package_name in filler_packages:
        for index in range(1, 51):
            _write(
                root
                / "packages"
                / package_name
                / "src"
                / package_name
                / "generated"
                / f"generated_{index:03d}.py",
                _filler_module(package_name, index),
            )
            _write(
                root / "packages" / package_name / "tests" / f"test_generated_{index:03d}.py",
                _filler_test(package_name, index),
            )

    for app_name in ("console", "worker"):
        for index in range(1, 61):
            _write(
                root / "apps" / app_name / "src" / app_name / "generated" / f"view_{index:03d}.py",
                dedent(
                    f'''
                    """Generated {app_name} application filler module {index:03d}."""

                    from __future__ import annotations


                    def build_{app_name}_view_{index:03d}(actor_id: str) -> dict[str, object]:
                        """Create a deterministic view payload for generated app scaffolding."""
                        return {{"actor_id": actor_id, "view": "{app_name}_{index:03d}"}}
                    '''
                ),
            )

    manifest = {
        "root": str(root),
        "file_count": sum(1 for path in root.rglob("*") if path.is_file()),
        "python_file_count": sum(1 for path in root.rglob("*.py")),
        "discovery_tasks": len(DISCOVERY_TASKS),
        "edit_tasks": len(EDIT_TASKS),
    }
    _write(root / "proxy_repo_manifest.json", json.dumps(manifest, indent=2))
    return manifest


def seed_git_history(repo_root: Path) -> list[str]:
    def run(*args: str) -> None:
        subprocess.run(args, cwd=repo_root, check=True, capture_output=True, text=True)

    run("git", "init")
    run("git", "config", "user.name", "Contextro Bench")
    run("git", "config", "user.email", "bench@example.com")
    run("git", "add", ".")
    run("git", "commit", "-m", "Initial public proxy benchmark repository")

    digest_path = repo_root / "packages/notifications/src/notifications/digest_scheduler.py"
    digest_text = digest_path.read_text(encoding="utf-8").replace(
        "DEFAULT_DIGEST_DELAY_MINUTES = 45",
        "DEFAULT_DIGEST_DELAY_MINUTES = 50",
    )
    digest_path.write_text(digest_text, encoding="utf-8")
    run("git", "add", str(digest_path.relative_to(repo_root)))
    run("git", "commit", "-m", "Adjust digest delay handling for billing alerts")

    alias_path = repo_root / "packages/partners/src/partners/aliases.py"
    alias_text = alias_path.read_text(encoding="utf-8").replace(
        'return alias.strip().lower().replace(" ", "-").replace("/", "-")',
        'return alias.strip().lower().replace(" ", "-").replace("/", "-").replace("_", "-")',
    )
    alias_path.write_text(alias_text, encoding="utf-8")
    run("git", "add", str(alias_path.relative_to(repo_root)))
    run("git", "commit", "-m", "Normalize partner alias keys before onboarding lookup")

    return [
        "Initial public proxy benchmark repository",
        "Adjust digest delay handling for billing alerts",
        "Normalize partner alias keys before onboarding lookup",
    ]


def export_task_catalogs(output_dir: Path) -> dict[str, Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    discovery_path = output_dir / "public-proxy-comparable-tasks.json"
    edit_path = output_dir / "public-proxy-edit-tasks.json"
    discovery_path.write_text(
        json.dumps([asdict(task) for task in DISCOVERY_TASKS], indent=2) + "\n",
        encoding="utf-8",
    )
    edit_path.write_text(
        json.dumps([asdict(task) for task in EDIT_TASKS], indent=2) + "\n",
        encoding="utf-8",
    )
    return {"discovery": discovery_path, "edit": edit_path}


def main() -> None:
    manifest = materialize_public_proxy_repo(DEFAULT_PROXY_REPO)
    catalogs = export_task_catalogs(ROOT / "docs" / "publication")
    print(json.dumps({"manifest": manifest, "catalogs": {k: str(v) for k, v in catalogs.items()}}, indent=2))


if __name__ == "__main__":
    main()
