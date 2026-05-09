"""
Controlled experiment: Contextro MCP vs no-MCP on the platform codebase.

Runs identical tasks in two arms:
  - Control: grep + file reads (simulating agent without MCP)
  - Treatment: Contextro MCP tool calls (real server)

Codebase: /Users/japneetkalkat/platform (~9400 source files)
"""

from __future__ import annotations

import asyncio
import json
import os
import subprocess
import sys
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

# Add contextro source to path
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src"))

PLATFORM_PATH = "/Users/japneetkalkat/platform"
VENV_PYTHON = "/Users/japneetkalkat/contextro/.venv/bin/python"
OUTPUT_DIR = ROOT / "scripts" / "experiment_results"

# Use isolated storage so we don't conflict with existing indexes
STORAGE_DIR = str(OUTPUT_DIR / ".contextro_experiment")


@dataclass
class TaskResult:
    task_id: str
    arm: str  # "control" or "mcp"
    category: str
    completed: bool = False
    wall_clock_ms: float = 0.0
    tokens_estimate: int = 0
    tool_calls: int = 0
    files_read: int = 0
    results_count: int = 0
    error: str = ""


@dataclass
class Task:
    id: str
    category: str
    description: str
    # For control arm
    grep_pattern: str
    # For MCP arm
    mcp_tool: str
    mcp_args: dict = field(default_factory=dict)


TASKS = [
    # ─── Core Discovery Tools ─────────────────────────────────────────────────
    # status / health (no args)
    Task("status_01", "server_ops", "Check server status",
         "", "status", {}),
    Task("health_01", "server_ops", "Health check",
         "", "health", {}),

    # ─── Symbol Discovery: find_symbol (8 tasks) ──────────────────────────────
    Task("sym_01", "symbol_discovery", "Find prepareIssueWorktree",
         "prepareIssueWorktree", "find_symbol", {"name": "prepareIssueWorktree"}),
    Task("sym_02", "symbol_discovery", "Find ensureTmuxSession",
         "ensureTmuxSession", "find_symbol", {"name": "ensureTmuxSession"}),
    Task("sym_03", "symbol_discovery", "Find getPartnerApiMcpConfig",
         "getPartnerApiMcpConfig", "find_symbol", {"name": "getPartnerApiMcpConfig"}),
    Task("sym_04", "symbol_discovery", "Find runJsonCommand",
         "runJsonCommand", "find_symbol", {"name": "runJsonCommand"}),
    Task("sym_05", "symbol_discovery", "Find buildSummary",
         "buildSummary", "find_symbol", {"name": "buildSummary"}),
    Task("sym_06", "symbol_discovery", "Find requirePageAccess",
         "requirePageAccess", "find_symbol", {"name": "requirePageAccess"}),
    Task("sym_07", "symbol_discovery", "Find getUserId",
         "getUserId", "find_symbol", {"name": "getUserId"}),
    Task("sym_08", "symbol_discovery", "Find useMapDerivedData",
         "useMapDerivedData", "find_symbol", {"name": "useMapDerivedData"}),

    # ─── Caller/Callee Tracing (6 tasks) ──────────────────────────────────────
    Task("call_01", "caller_tracing", "Who calls prepareIssueWorktree?",
         "prepareIssueWorktree", "find_callers", {"symbol_name": "prepareIssueWorktree"}),
    Task("call_02", "caller_tracing", "Who calls getPartnerApiMcpConfig?",
         "getPartnerApiMcpConfig", "find_callers", {"symbol_name": "getPartnerApiMcpConfig"}),
    Task("call_03", "caller_tracing", "Who calls runJsonCommand?",
         "runJsonCommand", "find_callers", {"symbol_name": "runJsonCommand"}),
    Task("call_04", "caller_tracing", "Who calls requirePageAccess?",
         "requirePageAccess", "find_callers", {"symbol_name": "requirePageAccess"}),
    Task("call_05", "caller_tracing", "What does prepareIssueWorktree call?",
         "prepareIssueWorktree", "find_callees", {"symbol_name": "prepareIssueWorktree"}),
    Task("call_06", "caller_tracing", "What does getUserId call?",
         "getUserId", "find_callees", {"symbol_name": "getUserId"}),

    # ─── Semantic Search (8 tasks) ────────────────────────────────────────────
    Task("search_01", "semantic_search", "Authentication flow",
         "auth", "search", {"query": "authentication flow login"}),
    Task("search_02", "semantic_search", "Agent workflow run",
         "agent.*workflow.*run", "search", {"query": "agent workflow run preparation"}),
    Task("search_03", "semantic_search", "Partner API MCP config",
         "partner.*api.*mcp", "search", {"query": "partner api mcp configuration"}),
    Task("search_04", "semantic_search", "SEO pipeline summary",
         "seo.*summary", "search", {"query": "seo pipeline summary"}),
    Task("search_05", "semantic_search", "Database migration",
         "migration", "search", {"query": "database migration strategy"}),
    Task("search_06", "semantic_search", "Rate limiting",
         "rate.*limit", "search", {"query": "rate limiting implementation"}),
    Task("search_07", "semantic_search", "WebSocket real-time",
         "websocket\\|real.time", "search", {"query": "websocket real-time updates"}),
    Task("search_08", "semantic_search", "Email notifications",
         "email.*notif", "search", {"query": "email notification system"}),

    # ─── Code Understanding: explain (6 tasks) ────────────────────────────────
    Task("explain_01", "code_understanding", "Explain requirePageAccess",
         "requirePageAccess", "explain", {"symbol_name": "requirePageAccess"}),
    Task("explain_02", "code_understanding", "Explain getUserId",
         "getUserId", "explain", {"symbol_name": "getUserId"}),
    Task("explain_03", "code_understanding", "Explain prepareIssueWorktree",
         "prepareIssueWorktree", "explain", {"symbol_name": "prepareIssueWorktree"}),
    Task("explain_04", "code_understanding", "Explain buildSummary",
         "buildSummary", "explain", {"symbol_name": "buildSummary"}),
    Task("explain_05", "code_understanding", "Explain getPartnerApiMcpConfig",
         "getPartnerApiMcpConfig", "explain", {"symbol_name": "getPartnerApiMcpConfig"}),
    Task("explain_06", "code_understanding", "Explain runJsonCommand",
         "runJsonCommand", "explain", {"symbol_name": "runJsonCommand"}),

    # ─── Impact Analysis (4 tasks) ────────────────────────────────────────────
    Task("impact_01", "impact_analysis", "Impact of changing requirePageAccess",
         "requirePageAccess", "impact", {"symbol_name": "requirePageAccess"}),
    Task("impact_02", "impact_analysis", "Impact of changing getUserId",
         "getUserId", "impact", {"symbol_name": "getUserId"}),
    Task("impact_03", "impact_analysis", "Impact of changing runJsonCommand",
         "runJsonCommand", "impact", {"symbol_name": "runJsonCommand"}),
    Task("impact_04", "impact_analysis", "Impact of changing buildSummary",
         "buildSummary", "impact", {"symbol_name": "buildSummary"}),

    # ─── Overview / Architecture / Analyze ────────────────────────────────────
    Task("overview_01", "project_structure", "Project overview",
         "", "overview", {}),
    Task("arch_01", "project_structure", "Architecture summary",
         "", "architecture", {}),
    Task("analyze_01", "project_structure", "Code analysis",
         "", "analyze", {}),

    # ─── Focus (file context) ─────────────────────────────────────────────────
    Task("focus_01", "focus", "Focus on auth callback",
         "auth.*callback", "focus", {"path": "apps/app/src/app/[locale]/(auth-shell)/auth/callback/page.tsx"}),
    Task("focus_02", "focus", "Focus on convex http",
         "http", "focus", {"path": "convex/http.ts"}),

    # ─── Git History Tools ────────────────────────────────────────────────────
    Task("commit_hist_01", "git_history", "Recent commits",
         "", "commit_history", {"limit": 10}),
    Task("commit_search_01", "git_history", "Search: auth changes",
         "auth", "commit_search", {"query": "authentication changes"}),
    Task("commit_search_02", "git_history", "Search: partner API",
         "partner", "commit_search", {"query": "partner api updates"}),

    # ─── Code Tool (AST operations) ──────────────────────────────────────────
    Task("code_symbols_01", "code_tool", "Search symbols: auth",
         "auth", "code", {"operation": "search_symbols", "symbol_name": "auth"}),
    Task("code_symbols_02", "code_tool", "Lookup symbols batch",
         "getUserId", "code", {"operation": "lookup_symbols", "symbols": "getUserId,requirePageAccess,buildSummary"}),
    Task("code_doc_01", "code_tool", "Document symbols in file",
         "", "code", {"operation": "get_document_symbols", "file_path": "convex/http.ts"}),
    Task("code_map_01", "code_tool", "Codebase map: convex",
         "", "code", {"operation": "search_codebase_map", "path": "convex/modules"}),
    Task("code_pattern_01", "code_tool", "Pattern search: export default",
         "export default", "code", {"operation": "pattern_search", "pattern": "export default function $NAME($$$) { $$$ }", "language": "typescript"}),

    # ─── Dead Code / Circular Deps / Coverage ─────────────────────────────────
    Task("dead_code_01", "static_analysis", "Dead code detection",
         "", "dead_code", {}),
    Task("circular_01", "static_analysis", "Circular dependencies",
         "", "circular_dependencies", {}),
    Task("coverage_01", "static_analysis", "Test coverage map",
         "", "test_coverage_map", {}),
    Task("audit_01", "static_analysis", "Full audit",
         "", "audit", {}),

    # ─── Memory Tools ─────────────────────────────────────────────────────────
    Task("remember_01", "memory", "Store a memory",
         "", "remember", {"content": "Experiment benchmark note: platform uses Convex + Next.js", "memory_type": "note", "tags": "experiment"}),
    Task("recall_01", "memory", "Recall memory",
         "", "recall", {"query": "platform architecture"}),
    Task("forget_01", "memory", "Forget experiment memories",
         "", "forget", {"tags": "experiment"}),

    # ─── Knowledge Tools ──────────────────────────────────────────────────────
    Task("knowledge_show_01", "knowledge", "Show knowledge bases",
         "", "knowledge", {"command": "show"}),

    # ─── Session Tools ────────────────────────────────────────────────────────
    Task("snapshot_01", "session", "Session snapshot",
         "", "session_snapshot", {}),
    Task("restore_01", "session", "Restore context",
         "", "restore", {}),
    Task("compact_01", "session", "Compact session",
         "", "compact", {"content": "Experiment session: benchmarking all 35 tools on platform codebase"}),

    # ─── Introspect ───────────────────────────────────────────────────────────
    Task("introspect_01", "introspect", "Introspect: available tools",
         "", "introspect", {"query": "what tools are available"}),

    # ─── Repo Management ──────────────────────────────────────────────────────
    Task("repo_status_01", "repo_mgmt", "Repo status",
         "", "repo_status", {}),
]


# ─── Control Arm: grep + file reads ───────────────────────────────────────────

def run_control_arm(task: Task) -> TaskResult:
    """Simulate agent without MCP: grep for pattern, read matching files."""
    start = time.perf_counter()
    files_read = 0
    tokens = 0
    tool_calls = 0
    results_count = 0

    # Tasks with no grep equivalent — agent would have no way to do this
    if not task.grep_pattern:
        elapsed = (time.perf_counter() - start) * 1000
        return TaskResult(
            task_id=task.id, arm="control", category=task.category,
            completed=True, wall_clock_ms=round(elapsed, 2),
            tokens_estimate=0, tool_calls=0, files_read=0,
            results_count=0, error="no_equivalent",
        )

    try:
        # Step 1: grep for the pattern across the codebase
        tool_calls += 1
        grep_cmd = [
            "grep", "-rl",
            "--include=*.ts", "--include=*.tsx",
            "--include=*.js", "--include=*.mjs",
            "--exclude-dir=node_modules", "--exclude-dir=.git",
            "--exclude-dir=_generated", "--exclude-dir=.next",
            task.grep_pattern, PLATFORM_PATH
        ]
        grep_result = subprocess.run(
            grep_cmd, capture_output=True, text=True, timeout=15,
            env={**os.environ, "LC_ALL": "C"}
        )
        matching_files = [f for f in grep_result.stdout.strip().split("\n") if f][:10]
        results_count = len(matching_files)

        # Tokens for grep output (file paths)
        tokens += len(grep_result.stdout) // 4

        # Step 2: Read up to 5 matching files
        files_to_read = matching_files[:5]
        for fpath in files_to_read:
            tool_calls += 1
            files_read += 1
            try:
                content = Path(fpath).read_text(errors="ignore")
                file_tokens = len(content) // 4
                tokens += file_tokens
            except (OSError, PermissionError):
                pass

    except subprocess.TimeoutExpired:
        tokens = 3000
        files_read = 3
        tool_calls = 4
    except Exception as e:
        return TaskResult(
            task_id=task.id, arm="control", category=task.category,
            error=str(e)
        )

    elapsed = (time.perf_counter() - start) * 1000

    return TaskResult(
        task_id=task.id, arm="control", category=task.category,
        completed=True, wall_clock_ms=round(elapsed, 2),
        tokens_estimate=tokens, tool_calls=tool_calls,
        files_read=files_read, results_count=results_count,
    )


# ─── Treatment Arm: Contextro MCP ────────────────────────────────────────────

def _configure_environment(storage_dir: str):
    """Set environment for isolated experiment."""
    os.environ["CTX_STORAGE_DIR"] = storage_dir
    os.environ["CTX_LOG_LEVEL"] = "WARNING"
    os.environ["CTX_EMBEDDING_MODEL"] = "potion-code-16m"


def _reset_runtime():
    """Reset and create a fresh MCP server instance."""
    import contextro_mcp.server as server_module
    from contextro_mcp.config import reset_settings
    from contextro_mcp.state import reset_state

    reset_settings()
    reset_state()
    server_module._pipeline = None
    server_module._index_job = {}
    return server_module.create_server(), server_module


async def index_and_wait(mcp, server_module, path: str, timeout: int = 300) -> dict:
    """Index the codebase and wait for completion."""
    from benchmark_utils import call_tool

    result = await call_tool(mcp, "index", {"path": path})
    if result.get("status") != "indexing":
        return result

    # Poll for completion
    for _ in range(timeout * 2):
        await asyncio.sleep(0.5)
        with server_module._index_job_lock:
            status = server_module._index_job.get("status")
            if status == "done":
                return server_module._index_job.get("result", {})
            if status == "error":
                return {"error": server_module._index_job.get("error", "unknown")}

    return {"error": "timeout"}


async def run_mcp_arm(task: Task, mcp) -> TaskResult:
    """Run a task using Contextro MCP tools."""
    from benchmark_utils import call_tool, estimate_tokens

    start = time.perf_counter()

    result = await call_tool(mcp, task.mcp_tool, task.mcp_args)
    elapsed = (time.perf_counter() - start) * 1000

    has_error = "error" in result and result["error"]
    tokens = estimate_tokens(result)

    # Count results
    results_count = 0
    if isinstance(result, dict):
        for key in ("callers", "callees", "results", "affected", "direct_callers"):
            if key in result and isinstance(result[key], list):
                results_count = len(result[key])
                break

    return TaskResult(
        task_id=task.id, arm="mcp", category=task.category,
        completed=not has_error, wall_clock_ms=round(elapsed, 2),
        tokens_estimate=tokens, tool_calls=1,
        files_read=0, results_count=results_count,
        error=str(result.get("error", "")) if has_error else "",
    )


# ─── Analysis ─────────────────────────────────────────────────────────────────

def analyze_results(results: list[TaskResult]) -> dict:
    """Compute summary statistics."""
    control = [r for r in results if r.arm == "control"]
    mcp = [r for r in results if r.arm == "mcp"]

    def stats(items, key):
        vals = [getattr(r, key) for r in items if r.completed]
        if not vals:
            return {"count": 0, "sum": 0, "mean": 0, "median": 0, "p95": 0}
        vals.sort()
        n = len(vals)
        return {
            "count": n,
            "sum": sum(vals),
            "mean": round(sum(vals) / n, 1),
            "median": vals[n // 2],
            "p95": vals[min(n - 1, int(n * 0.95))],
        }

    # Per-category breakdown
    categories = sorted(set(r.category for r in results))
    by_category = {}
    for cat in categories:
        cat_ctrl = [r for r in control if r.category == cat and r.completed]
        cat_mcp = [r for r in mcp if r.category == cat and r.completed]
        ctrl_tokens = sum(r.tokens_estimate for r in cat_ctrl)
        mcp_tokens = sum(r.tokens_estimate for r in cat_mcp)
        by_category[cat] = {
            "tasks": len(cat_ctrl),
            "control_tokens": ctrl_tokens,
            "mcp_tokens": mcp_tokens,
            "token_reduction_pct": round((1 - mcp_tokens / max(ctrl_tokens, 1)) * 100, 1),
            "control_mean_ms": round(sum(r.wall_clock_ms for r in cat_ctrl) / max(len(cat_ctrl), 1), 1),
            "mcp_mean_ms": round(sum(r.wall_clock_ms for r in cat_mcp) / max(len(cat_mcp), 1), 1),
        }

    ctrl_total_tokens = sum(r.tokens_estimate for r in control if r.completed)
    mcp_total_tokens = sum(r.tokens_estimate for r in mcp if r.completed)

    return {
        "total_tasks": len(TASKS),
        "control_completed": sum(1 for r in control if r.completed),
        "mcp_completed": sum(1 for r in mcp if r.completed),
        "tokens": {
            "control": stats(control, "tokens_estimate"),
            "mcp": stats(mcp, "tokens_estimate"),
            "total_reduction_pct": round((1 - mcp_total_tokens / max(ctrl_total_tokens, 1)) * 100, 1),
        },
        "latency_ms": {
            "control": stats(control, "wall_clock_ms"),
            "mcp": stats(mcp, "wall_clock_ms"),
        },
        "tool_calls": {
            "control": stats(control, "tool_calls"),
            "mcp": stats(mcp, "tool_calls"),
        },
        "files_read": {
            "control": stats(control, "files_read"),
            "mcp": stats(mcp, "files_read"),
        },
        "by_category": by_category,
    }


# ─── Main ─────────────────────────────────────────────────────────────────────

async def main():
    print("=" * 60)
    print("  CONTEXTRO MCP vs NO-MCP CONTROLLED EXPERIMENT")
    print("=" * 60)
    print(f"\n  Codebase: {PLATFORM_PATH}")
    print(f"  Tasks:    {len(TASKS)}")
    print(f"  Output:   {OUTPUT_DIR}\n")

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    # Configure isolated environment
    _configure_environment(STORAGE_DIR)

    # Initialize MCP server
    print("─── Initializing Contextro MCP ───")
    mcp, server_module = _reset_runtime()

    # Index the platform codebase
    print(f"  Indexing {PLATFORM_PATH}...")
    index_start = time.perf_counter()
    index_result = await index_and_wait(mcp, server_module, PLATFORM_PATH)
    index_time = time.perf_counter() - index_start
    print(f"  Index complete: {index_result.get('total_files', '?')} files in {index_time:.1f}s")
    print("─── MCP Ready ───\n")

    results: list[TaskResult] = []

    # Run all tasks
    print("─── Running Experiments ───\n")
    print(f"{'Task':<12} {'Category':<20} {'Ctrl Tokens':>12} {'MCP Tokens':>11} {'Savings':>8}")
    print("─" * 65)

    for task in TASKS:
        # Control arm
        ctrl_result = run_control_arm(task)
        results.append(ctrl_result)

        # MCP arm
        mcp_result = await run_mcp_arm(task, mcp)
        results.append(mcp_result)

        # Print row
        if ctrl_result.completed and mcp_result.completed:
            savings = round((1 - mcp_result.tokens_estimate / max(ctrl_result.tokens_estimate, 1)) * 100)
            print(f"{task.id:<12} {task.category:<20} {ctrl_result.tokens_estimate:>12,} {mcp_result.tokens_estimate:>11,} {savings:>7}%")
        else:
            err = ctrl_result.error or mcp_result.error
            print(f"{task.id:<12} {task.category:<20} {'ERROR':>12} {'':>11} {err[:20]}")

    # Analyze
    print("\n─── Analysis ───\n")
    summary = analyze_results(results)

    # Print summary
    print(f"  Tasks completed: {summary['control_completed']}/{summary['total_tasks']} (control), "
          f"{summary['mcp_completed']}/{summary['total_tasks']} (MCP)")
    print()
    print(f"  {'Metric':<20} {'Control':>12} {'MCP':>12} {'Reduction':>10}")
    print(f"  {'─'*54}")
    print(f"  {'Total tokens':<20} {summary['tokens']['control']['sum']:>12,} {summary['tokens']['mcp']['sum']:>12,} {summary['tokens']['total_reduction_pct']:>9.1f}%")
    print(f"  {'Mean tokens/task':<20} {summary['tokens']['control']['mean']:>12,.0f} {summary['tokens']['mcp']['mean']:>12,.0f}")
    print(f"  {'Median latency ms':<20} {summary['latency_ms']['control']['median']:>12,.1f} {summary['latency_ms']['mcp']['median']:>12,.1f}")
    print(f"  {'Mean tool calls':<20} {summary['tool_calls']['control']['mean']:>12,.1f} {summary['tool_calls']['mcp']['mean']:>12,.1f}")
    print(f"  {'Mean files read':<20} {summary['files_read']['control']['mean']:>12,.1f} {summary['files_read']['mcp']['mean']:>12,.1f}")

    print("\n  By Category:")
    print(f"  {'Category':<22} {'Ctrl Tok':>9} {'MCP Tok':>9} {'Reduction':>10}")
    print(f"  {'─'*52}")
    for cat, data in summary["by_category"].items():
        print(f"  {cat:<22} {data['control_tokens']:>9,} {data['mcp_tokens']:>9,} {data['token_reduction_pct']:>9.1f}%")

    # Save results
    results_data = [asdict(r) for r in results]
    (OUTPUT_DIR / "results.json").write_text(json.dumps(results_data, indent=2))
    (OUTPUT_DIR / "summary.json").write_text(json.dumps(summary, indent=2))
    (OUTPUT_DIR / "config.json").write_text(json.dumps({
        "codebase": PLATFORM_PATH,
        "tasks": len(TASKS),
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "storage_dir": STORAGE_DIR,
    }, indent=2))

    print(f"\n  Results saved to: {OUTPUT_DIR}/")
    print("=" * 60)


if __name__ == "__main__":
    asyncio.run(main())
