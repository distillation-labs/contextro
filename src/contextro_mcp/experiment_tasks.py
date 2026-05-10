"""Sanitized public task inventory and export helpers for the paired study."""

from __future__ import annotations

import json
from collections.abc import Sequence
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

FULL_TASK_CATALOG_FILENAME = "paired-study-tasks.json"
COMPARABLE_TASK_CATALOG_FILENAME = "paired-study-comparable-tasks.json"

REDACTION_NOTE = (
    "This public paired-study inventory is sanitized. Private symbol names, file "
    "paths, free-text prompts, and machine-local content have been replaced with "
    "placeholders while preserving task-family counts, categories, and tool coverage."
)

SYMBOL_01 = "<private_symbol_01>"
SYMBOL_02 = "<private_symbol_02>"
SYMBOL_03 = "<private_symbol_03>"
SYMBOL_04 = "<private_symbol_04>"
SYMBOL_05 = "<private_symbol_05>"
SYMBOL_06 = "<private_symbol_06>"
SYMBOL_07 = "<private_symbol_07>"
SYMBOL_08 = "<private_symbol_08>"

PATH_01 = "<private_path_01>"
PATH_02 = "<private_path_02>"
PATH_GROUP_01 = "<private_path_group_01>"

NOTE_01 = "<private_note_01>"
NOTE_02 = "<private_note_02>"
MEMORY_QUERY_01 = "<private_query_01>"


@dataclass(frozen=True)
class Task:
    id: str
    category: str
    description: str
    grep_pattern: str
    mcp_tool: str
    mcp_args: dict[str, Any] = field(default_factory=dict)

    @property
    def control_equivalent(self) -> bool:
        return bool(self.grep_pattern)


TASKS: tuple[Task, ...] = (
    Task("status_01", "server_ops", "Check server status", "", "status", {}),
    Task("health_01", "server_ops", "Health check", "", "health", {}),
    Task(
        "sym_01",
        "symbol_discovery",
        f"Find {SYMBOL_01}",
        SYMBOL_01,
        "find_symbol",
        {"name": SYMBOL_01},
    ),
    Task(
        "sym_02",
        "symbol_discovery",
        f"Find {SYMBOL_02}",
        SYMBOL_02,
        "find_symbol",
        {"name": SYMBOL_02},
    ),
    Task(
        "sym_03",
        "symbol_discovery",
        f"Find {SYMBOL_03}",
        SYMBOL_03,
        "find_symbol",
        {"name": SYMBOL_03},
    ),
    Task(
        "sym_04",
        "symbol_discovery",
        f"Find {SYMBOL_04}",
        SYMBOL_04,
        "find_symbol",
        {"name": SYMBOL_04},
    ),
    Task(
        "sym_05",
        "symbol_discovery",
        f"Find {SYMBOL_05}",
        SYMBOL_05,
        "find_symbol",
        {"name": SYMBOL_05},
    ),
    Task(
        "sym_06",
        "symbol_discovery",
        f"Find {SYMBOL_06}",
        SYMBOL_06,
        "find_symbol",
        {"name": SYMBOL_06},
    ),
    Task(
        "sym_07",
        "symbol_discovery",
        f"Find {SYMBOL_07}",
        SYMBOL_07,
        "find_symbol",
        {"name": SYMBOL_07},
    ),
    Task(
        "sym_08",
        "symbol_discovery",
        f"Find {SYMBOL_08}",
        SYMBOL_08,
        "find_symbol",
        {"name": SYMBOL_08},
    ),
    Task(
        "call_01",
        "caller_tracing",
        f"Who calls {SYMBOL_01}?",
        SYMBOL_01,
        "find_callers",
        {"symbol_name": SYMBOL_01},
    ),
    Task(
        "call_02",
        "caller_tracing",
        f"Who calls {SYMBOL_03}?",
        SYMBOL_03,
        "find_callers",
        {"symbol_name": SYMBOL_03},
    ),
    Task(
        "call_03",
        "caller_tracing",
        f"Who calls {SYMBOL_04}?",
        SYMBOL_04,
        "find_callers",
        {"symbol_name": SYMBOL_04},
    ),
    Task(
        "call_04",
        "caller_tracing",
        f"Who calls {SYMBOL_06}?",
        SYMBOL_06,
        "find_callers",
        {"symbol_name": SYMBOL_06},
    ),
    Task(
        "call_05",
        "caller_tracing",
        f"What does {SYMBOL_01} call?",
        SYMBOL_01,
        "find_callees",
        {"symbol_name": SYMBOL_01},
    ),
    Task(
        "call_06",
        "caller_tracing",
        f"What does {SYMBOL_07} call?",
        SYMBOL_07,
        "find_callees",
        {"symbol_name": SYMBOL_07},
    ),
    Task(
        "search_01",
        "semantic_search",
        "Semantic search task 01",
        "<private_search_pattern_01>",
        "search",
        {"query": "<private_search_query_01>"},
    ),
    Task(
        "search_02",
        "semantic_search",
        "Semantic search task 02",
        "<private_search_pattern_02>",
        "search",
        {"query": "<private_search_query_02>"},
    ),
    Task(
        "search_03",
        "semantic_search",
        "Semantic search task 03",
        "<private_search_pattern_03>",
        "search",
        {"query": "<private_search_query_03>"},
    ),
    Task(
        "search_04",
        "semantic_search",
        "Semantic search task 04",
        "<private_search_pattern_04>",
        "search",
        {"query": "<private_search_query_04>"},
    ),
    Task(
        "search_05",
        "semantic_search",
        "Semantic search task 05",
        "<private_search_pattern_05>",
        "search",
        {"query": "<private_search_query_05>"},
    ),
    Task(
        "search_06",
        "semantic_search",
        "Semantic search task 06",
        "<private_search_pattern_06>",
        "search",
        {"query": "<private_search_query_06>"},
    ),
    Task(
        "search_07",
        "semantic_search",
        "Semantic search task 07",
        "<private_search_pattern_07>",
        "search",
        {"query": "<private_search_query_07>"},
    ),
    Task(
        "search_08",
        "semantic_search",
        "Semantic search task 08",
        "<private_search_pattern_08>",
        "search",
        {"query": "<private_search_query_08>"},
    ),
    Task(
        "explain_01",
        "code_understanding",
        f"Explain {SYMBOL_06}",
        SYMBOL_06,
        "explain",
        {"symbol_name": SYMBOL_06},
    ),
    Task(
        "explain_02",
        "code_understanding",
        f"Explain {SYMBOL_07}",
        SYMBOL_07,
        "explain",
        {"symbol_name": SYMBOL_07},
    ),
    Task(
        "explain_03",
        "code_understanding",
        f"Explain {SYMBOL_01}",
        SYMBOL_01,
        "explain",
        {"symbol_name": SYMBOL_01},
    ),
    Task(
        "explain_04",
        "code_understanding",
        f"Explain {SYMBOL_05}",
        SYMBOL_05,
        "explain",
        {"symbol_name": SYMBOL_05},
    ),
    Task(
        "explain_05",
        "code_understanding",
        f"Explain {SYMBOL_03}",
        SYMBOL_03,
        "explain",
        {"symbol_name": SYMBOL_03},
    ),
    Task(
        "explain_06",
        "code_understanding",
        f"Explain {SYMBOL_04}",
        SYMBOL_04,
        "explain",
        {"symbol_name": SYMBOL_04},
    ),
    Task(
        "impact_01",
        "impact_analysis",
        f"Impact of changing {SYMBOL_06}",
        SYMBOL_06,
        "impact",
        {"symbol_name": SYMBOL_06},
    ),
    Task(
        "impact_02",
        "impact_analysis",
        f"Impact of changing {SYMBOL_07}",
        SYMBOL_07,
        "impact",
        {"symbol_name": SYMBOL_07},
    ),
    Task(
        "impact_03",
        "impact_analysis",
        f"Impact of changing {SYMBOL_04}",
        SYMBOL_04,
        "impact",
        {"symbol_name": SYMBOL_04},
    ),
    Task(
        "impact_04",
        "impact_analysis",
        f"Impact of changing {SYMBOL_05}",
        SYMBOL_05,
        "impact",
        {"symbol_name": SYMBOL_05},
    ),
    Task("overview_01", "project_structure", "Project overview", "", "overview", {}),
    Task("arch_01", "project_structure", "Architecture summary", "", "architecture", {}),
    Task("analyze_01", "project_structure", "Code analysis", "", "analyze", {}),
    Task(
        "focus_01",
        "focus",
        "Focus on private file 01",
        "<private_focus_pattern_01>",
        "focus",
        {"path": PATH_01},
    ),
    Task(
        "focus_02",
        "focus",
        "Focus on private file 02",
        "<private_focus_pattern_02>",
        "focus",
        {"path": PATH_02},
    ),
    Task("commit_hist_01", "git_history", "Recent commits", "", "commit_history", {"limit": 10}),
    Task(
        "commit_search_01",
        "git_history",
        "Search private history topic 01",
        "<private_commit_pattern_01>",
        "commit_search",
        {"query": "<private_commit_query_01>"},
    ),
    Task(
        "commit_search_02",
        "git_history",
        "Search private history topic 02",
        "<private_commit_pattern_02>",
        "commit_search",
        {"query": "<private_commit_query_02>"},
    ),
    Task(
        "code_symbols_01",
        "code_tool",
        "Search symbols: private cluster 01",
        "<private_code_pattern_01>",
        "code",
        {"operation": "search_symbols", "symbol_name": "<private_symbol_group_01>"},
    ),
    Task(
        "code_symbols_02",
        "code_tool",
        "Lookup symbols batch",
        SYMBOL_07,
        "code",
        {
            "operation": "lookup_symbols",
            "symbols": ",".join((SYMBOL_07, SYMBOL_06, SYMBOL_05)),
        },
    ),
    Task(
        "code_doc_01",
        "code_tool",
        "Document symbols in file",
        "",
        "code",
        {"operation": "get_document_symbols", "file_path": PATH_02},
    ),
    Task(
        "code_map_01",
        "code_tool",
        "Codebase map: private area 01",
        "",
        "code",
        {"operation": "search_codebase_map", "path": PATH_GROUP_01},
    ),
    Task(
        "code_pattern_01",
        "code_tool",
        "Pattern search: export default",
        "export default",
        "code",
        {
            "operation": "pattern_search",
            "pattern": "export default function $NAME($$$) { $$$ }",
            "language": "typescript",
        },
    ),
    Task("dead_code_01", "static_analysis", "Dead code detection", "", "dead_code", {}),
    Task(
        "circular_01",
        "static_analysis",
        "Circular dependencies",
        "",
        "circular_dependencies",
        {},
    ),
    Task("coverage_01", "static_analysis", "Test coverage map", "", "test_coverage_map", {}),
    Task("audit_01", "static_analysis", "Full audit", "", "audit", {}),
    Task(
        "remember_01",
        "memory",
        "Store a memory",
        "",
        "remember",
        {"content": NOTE_01, "memory_type": "note", "tags": "experiment"},
    ),
    Task("recall_01", "memory", "Recall memory", "", "recall", {"query": MEMORY_QUERY_01}),
    Task(
        "forget_01",
        "memory",
        "Forget experiment memories",
        "",
        "forget",
        {"tags": "experiment"},
    ),
    Task(
        "knowledge_show_01",
        "knowledge",
        "Show knowledge bases",
        "",
        "knowledge",
        {"command": "show"},
    ),
    Task("snapshot_01", "session", "Session snapshot", "", "session_snapshot", {}),
    Task("restore_01", "session", "Restore context", "", "restore", {}),
    Task(
        "compact_01",
        "session",
        "Compact session",
        "",
        "compact",
        {"content": NOTE_02},
    ),
    Task(
        "introspect_01",
        "introspect",
        "Introspect: available tools",
        "",
        "introspect",
        {"query": "what tools are available"},
    ),
    Task("repo_status_01", "repo_mgmt", "Repo status", "", "repo_status", {}),
)


def comparable_tasks(tasks: Sequence[Task] = TASKS) -> list[Task]:
    return [task for task in tasks if task.control_equivalent]


def mcp_native_tasks(tasks: Sequence[Task] = TASKS) -> list[Task]:
    return [task for task in tasks if not task.control_equivalent]


def _task_record(task: Task) -> dict[str, Any]:
    record = asdict(task)
    record["control_equivalent"] = task.control_equivalent
    return record


def build_full_task_catalog(tasks: Sequence[Task] = TASKS) -> dict[str, Any]:
    categories = sorted({task.category for task in tasks})
    comparable = comparable_tasks(tasks)
    mcp_native = mcp_native_tasks(tasks)
    return {
        "suite": "contextro_paired_study",
        "catalog": "full",
        "redacted": True,
        "redaction_note": REDACTION_NOTE,
        "task_count": len(tasks),
        "category_count": len(categories),
        "categories": categories,
        "comparable_task_count": len(comparable),
        "mcp_native_task_count": len(mcp_native),
        "selection_criteria": (
            "Sanitized public 60-task paired-study inventory preserving task-family "
            "counts, categories, and tool coverage."
        ),
        "tasks": [_task_record(task) for task in tasks],
    }


def build_comparable_task_catalog(tasks: Sequence[Task] = TASKS) -> dict[str, Any]:
    comparable = comparable_tasks(tasks)
    categories = sorted({task.category for task in comparable})
    excluded = mcp_native_tasks(tasks)
    return {
        "suite": "contextro_paired_study",
        "catalog": "comparable",
        "redacted": True,
        "redaction_note": REDACTION_NOTE,
        "task_count": len(comparable),
        "full_suite_task_count": len(tasks),
        "category_count": len(categories),
        "categories": categories,
        "selection_criteria": (
            "Sanitized public comparable subset preserving the original scripted "
            "control-equivalent task count."
        ),
        "excluded_task_ids": [task.id for task in excluded],
        "tasks": [_task_record(task) for task in comparable],
    }


def write_task_catalogs(output_dir: Path, tasks: Sequence[Task] = TASKS) -> dict[str, Path]:
    output_dir.mkdir(parents=True, exist_ok=True)

    full_path = output_dir / FULL_TASK_CATALOG_FILENAME
    comparable_path = output_dir / COMPARABLE_TASK_CATALOG_FILENAME

    full_path.write_text(json.dumps(build_full_task_catalog(tasks), indent=2) + "\n")
    comparable_path.write_text(json.dumps(build_comparable_task_catalog(tasks), indent=2) + "\n")

    return {
        "full": full_path,
        "comparable": comparable_path,
    }
