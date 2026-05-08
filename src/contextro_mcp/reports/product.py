"""Productized report builders for focus, restore, audit, and docs."""

from __future__ import annotations

from collections import Counter
from pathlib import Path
from typing import Any

from contextro_mcp.analysis.code_analyzer import CodeAnalyzer
from contextro_mcp.analysis.repository_map import (
    RepositoryMap,
    build_repository_map,
    layer_hint,
    related_tests,
    top_degree_files,
)
from contextro_mcp.analysis.static_analysis import (
    analyze_circular_dependencies,
    analyze_dead_code,
    analyze_test_coverage_map,
)
from contextro_mcp.config import get_settings
from contextro_mcp.execution.ast_compression import compress_snippet

AUDIT_SCHEMA_VERSION = 1
DOCS_SECTION_ORDER = ("index.md", "architecture.md", "audit.md", "llms.txt")


def get_repository_map_for_state(state) -> RepositoryMap:
    """Build or reuse a cached repository map for the current state."""
    root = _require_root(state)
    graph = state.graph_engine
    if graph is None:
        raise ValueError("No graph available. Run index first.")

    signature = (
        str(root),
        len(getattr(graph, "nodes", {})),
        len(getattr(graph, "relationships", {})),
    )
    cache = getattr(state, "_repository_map_cache", None)
    if cache and cache.get("signature") == signature:
        return cache["repo_map"]

    repo_map = build_repository_map(root, graph, get_settings())
    state._repository_map_cache = {"signature": signature, "repo_map": repo_map}
    return repo_map


def get_static_analysis_bundle(state) -> dict[str, Any]:
    """Build or reuse the current static analysis bundle."""
    root = _require_root(state)
    graph = state.graph_engine
    repo_map = get_repository_map_for_state(state)
    signature = (
        str(root),
        len(getattr(graph, "nodes", {})),
        len(getattr(graph, "relationships", {})),
    )
    cache = getattr(state, "_static_analysis_cache", None)
    if cache and cache.get("signature") == signature:
        return cache["bundle"]

    bundle = {
        "dead_code": analyze_dead_code(repo_map, graph),
        "circular_dependencies": analyze_circular_dependencies(repo_map),
        "test_coverage_map": analyze_test_coverage_map(repo_map, graph),
    }
    state._static_analysis_cache = {"signature": signature, "bundle": bundle}
    return bundle


def build_focus_report(state, target_path: str, *, include_code: bool = True) -> dict[str, Any]:
    """Build a low-token context slice for a single file."""
    root = _require_root(state)
    graph = state.graph_engine
    repo_map = get_repository_map_for_state(state)
    relative_path = _resolve_relative_path(root, target_path)
    module = repo_map.modules.get(relative_path)
    if module is None:
        raise ValueError(f"File not found in indexed codebase: {target_path}")

    symbols = []
    for symbol_id in module.symbol_ids:
        node = graph.get_node(symbol_id)
        if node is None:
            continue
        symbols.append(
            {
                "name": node.name,
                "type": node.node_type.value,
                "line": node.location.start_line,
                "line_count": node.line_count,
            }
        )
    symbols.sort(key=lambda item: (item["line"], item["name"]))

    direct_impact = sorted(set(module.dependents) | set(module.called_by))
    report = {
        "path": relative_path,
        "role": f"{layer_hint(relative_path)} module",
        "entry_point": module.is_entry,
        "test_file": module.is_test,
        "symbols": symbols[:15],
        "imports": list(module.imports[:10]),
        "imported_by": list(module.dependents[:10]),
        "calls": list(module.calls[:10]),
        "called_by": list(module.called_by[:10]),
        "nearby_tests": list(related_tests(repo_map, relative_path)[:10]),
        "blast_radius": {
            "direct_files": len(direct_impact),
            "top_dependents": direct_impact[:10],
        },
        "architecture_hint": layer_hint(relative_path),
    }
    if include_code:
        source_text = _safe_read(root / relative_path)
        report["code_preview"] = compress_snippet(source_text[:8000], module.language)
    return report


def build_restore_report(state) -> dict[str, Any]:
    """Build a project re-entry summary."""
    root = _require_root(state)
    repo_map = get_repository_map_for_state(state)
    audit = build_audit_report(state)

    layer_counts = Counter(layer_hint(path) for path in repo_map.modules)
    snapshot = None
    if hasattr(state, "_session_tracker") and state._session_tracker is not None:
        snapshot = state._session_tracker.get_snapshot(max_tokens=300)

    archive = None
    if hasattr(state, "_compaction_archive") and state._compaction_archive is not None:
        archive = {"entries": state._compaction_archive.size}

    return {
        "project": {
            "name": root.name,
            "root": str(root),
            "files": len(repo_map.modules),
        },
        "entry_points": list(repo_map.entry_points[:10]),
        "layers": dict(layer_counts.most_common(8)),
        "hub_files": top_degree_files(repo_map, limit=8),
        "risk_summary": audit["summary"],
        "recent_session": snapshot or {},
        "archive": archive or {},
        "suggested_next_commands": [
            "search(query='...')",
            "focus(path='path/to/file')",
            "impact(symbol_name='...')",
            "audit()",
        ],
    }


def build_audit_report(state) -> dict[str, Any]:
    """Build a packaged audit report."""
    graph = state.graph_engine
    repo_map = get_repository_map_for_state(state)
    analyzer = CodeAnalyzer(graph)
    static_bundle = get_static_analysis_bundle(state)
    complexity = analyzer.analyze_complexity()
    quality = analyzer.calculate_quality_metrics()
    hub_risks = top_degree_files(repo_map, limit=10)
    uncovered_files = static_bundle["test_coverage_map"]["uncovered_files"]
    uncovered_hubs = []
    for item in hub_risks:
        module = repo_map.modules.get(item["path"])
        if item["path"] not in uncovered_files or module is None or module.is_test:
            continue
        uncovered_hubs.append(item)

    summary = {
        "quality_score": quality["quality_score"],
        "maintainability_index": quality["maintainability_index"],
        "coverage_ratio": static_bundle["test_coverage_map"]["summary"]["coverage_ratio"],
        "dead_files": static_bundle["dead_code"]["summary"]["unused_files"],
        "dead_symbols": static_bundle["dead_code"]["summary"]["unused_symbols"],
        "circular_dependencies": static_bundle["circular_dependencies"]["summary"]["cycle_count"],
        "coverage_gaps": static_bundle["test_coverage_map"]["summary"]["uncovered_files"],
        "high_complexity_functions": len(complexity["high_complexity_functions"]),
        "blast_radius_hotspots": len([item for item in hub_risks if item["degree"] > 0]),
        "uncovered_hub_files": len(uncovered_hubs),
    }

    recommendation_details = []
    if summary["circular_dependencies"]:
        recommendation_details.append(
            {
                "priority": "high",
                "category": "cycles",
                "action": "Break file-level dependency cycles before expanding these modules.",
                "reason": (
                    "Cycles increase change risk and make module boundaries "
                    "harder to reason about."
                ),
                "evidence": {
                    "cycle_count": summary["circular_dependencies"],
                    "files": _cycle_files(
                        static_bundle["circular_dependencies"]["cycles"],
                        limit=6,
                    ),
                },
            }
        )
    if uncovered_hubs:
        recommendation_details.append(
            {
                "priority": "high",
                "category": "coverage",
                "action": (
                    "Add tests around uncovered production files with the widest "
                    "blast radius."
                ),
                "reason": "Uncovered hub files can break multiple dependents with little warning.",
                "evidence": {
                    "coverage_ratio": summary["coverage_ratio"],
                    "files": [item["path"] for item in uncovered_hubs[:5]],
                },
            }
        )
    elif summary["coverage_gaps"]:
        recommendation_details.append(
            {
                "priority": "medium",
                "category": "coverage",
                "action": "Add tests around uncovered production files.",
                "reason": (
                    "Static reachability found production files without "
                    "transitive test coverage."
                ),
                "evidence": {
                    "coverage_ratio": summary["coverage_ratio"],
                    "files": uncovered_files[:5],
                },
            }
        )
    if summary["dead_files"] or summary["dead_symbols"]:
        recommendation_details.append(
            {
                "priority": "medium",
                "category": "dead_code",
                "action": "Prune unreachable files and uncalled private symbols.",
                "reason": "Dead code increases maintenance cost and obscures the live graph.",
                "evidence": {
                    "unused_files": static_bundle["dead_code"]["unused_files"][:5],
                    "unused_symbols": [
                        item["location"]
                        for item in static_bundle["dead_code"]["unused_symbols"][:5]
                    ],
                },
            }
        )
    if complexity["high_complexity_functions"]:
        recommendation_details.append(
            {
                "priority": "medium",
                "category": "complexity",
                "action": "Refactor the highest-complexity functions before they become hubs.",
                "reason": (
                    "Complex functions are harder to change safely and often "
                    "accumulate callers."
                ),
                "evidence": {
                    "functions": [
                        item["location"] for item in complexity["high_complexity_functions"][:5]
                    ],
                },
            }
        )
    if not recommendation_details:
        recommendation_details.append(
            {
                "priority": "low",
                "category": "status",
                "action": "No urgent cleanup hotspots detected in the current static graph.",
                "reason": (
                    "The packaged graph checks did not flag cycles, dead code, "
                    "or coverage gaps."
                ),
                "evidence": {},
            }
        )
    recommendations = [item["action"] for item in recommendation_details]

    return {
        "report_type": "audit",
        "schema_version": AUDIT_SCHEMA_VERSION,
        "summary": summary,
        "quality": quality,
        "complexity": complexity,
        "blast_radius_hotspots": hub_risks,
        "hub_risks": hub_risks,
        "dead_code": static_bundle["dead_code"],
        "circular_dependencies": static_bundle["circular_dependencies"],
        "test_coverage_map": static_bundle["test_coverage_map"],
        "recommendation_details": recommendation_details,
        "recommendations": recommendations,
    }


def build_docs_sections(state) -> dict[str, str]:
    """Build the packaged docs bundle as markdown/plain-text sections."""
    root = _require_root(state)
    repo_map = get_repository_map_for_state(state)
    restore = build_restore_report(state)
    audit = build_audit_report(state)

    index_doc = "\n\n".join(
        [
            "# Contextro Docs Bundle",
            f"_Project:_ `{root.name}`",
            _section(
                "Bundle Contents",
                "\n".join(
                    [
                        "- [Architecture](architecture.md) — layers, entry points, and hub files.",
                        (
                            "- [Audit](audit.md) — prioritized risks, dead "
                            "code, cycles, and coverage gaps."
                        ),
                        "- [LLMs Context](llms.txt) — terse briefing for agents and scripts.",
                    ]
                ),
            ),
            _section(
                "Project Summary",
                "\n".join(
                    [
                        f"- **Root:** `{root}`",
                        f"- **Files Indexed:** {len(repo_map.modules)}",
                        f"- **Entry Points:** {len(restore['entry_points'])}",
                    ]
                ),
            ),
            _section("Entry Points", _code_list(restore["entry_points"][:10])),
            _section("Architecture Snapshot", _architecture_snapshot_markdown(restore)),
            _section("Audit Snapshot", _audit_summary_markdown(audit["summary"])),
            _section(
                "Recent Session Snapshot",
                _recent_session_markdown(
                    restore.get("recent_session", {}),
                    restore.get("archive", {}),
                ),
            ),
            _section(
                "Suggested Commands",
                _code_list(restore.get("suggested_next_commands", [])),
            ),
        ]
    )
    architecture_doc = "\n\n".join(
        [
            "# Contextro Architecture",
            _section(
                "Project",
                "\n".join(
                    [
                        f"- **Name:** {restore['project']['name']}",
                        f"- **Root:** `{restore['project']['root']}`",
                        f"- **Indexed Files:** {restore['project']['files']}",
                    ]
                ),
            ),
            _section("Entry Points", _code_list(restore["entry_points"][:12])),
            _section("Layer Breakdown", _mapping_bullets(restore["layers"])),
            _section("Hub Files", _hub_items_markdown(restore["hub_files"])),
            _section(
                "Suggested Commands",
                _code_list(restore.get("suggested_next_commands", [])),
            ),
        ]
    )
    audit_doc = "\n\n".join(
        [
            "# Contextro Audit",
            _section("Summary", _audit_summary_markdown(audit["summary"])),
            _section(
                "Quality Metrics",
                _ordered_summary_bullets(
                    audit["quality"],
                    ("quality_score", "maintainability_index", "documentation_ratio"),
                ),
            ),
            _section(
                "Complexity Metrics",
                _ordered_summary_bullets(
                    audit["complexity"],
                    ("total_functions", "average_complexity", "max_complexity"),
                ),
            ),
            _section(
                "Prioritized Recommendations",
                _recommendation_details_markdown(audit["recommendation_details"]),
            ),
            _section(
                "Blast Radius Hotspots",
                _hub_items_markdown(audit["blast_radius_hotspots"]),
            ),
            _section(
                "Complexity Hotspots",
                _complexity_items_markdown(audit["complexity"]["high_complexity_functions"]),
            ),
            _section(
                "Dead Code",
                "\n\n".join(
                    [
                        "### Unused Files\n" + _code_list(audit["dead_code"]["unused_files"][:10]),
                        "### Unused Symbols\n"
                        + _unused_symbol_markdown(audit["dead_code"]["unused_symbols"][:10]),
                    ]
                ),
            ),
            _section(
                "Circular Dependencies",
                _cycles_markdown(audit["circular_dependencies"]["cycles"][:10]),
            ),
            _section(
                "Test Coverage Gaps",
                "\n\n".join(
                    [
                        "### Uncovered Files\n"
                        + _code_list(audit["test_coverage_map"]["uncovered_files"][:10]),
                        "### Uncovered Symbols\n"
                        + _unused_symbol_markdown(
                            audit["test_coverage_map"]["uncovered_symbols"][:10]
                        ),
                        f"_Note:_ {audit['test_coverage_map']['note']}",
                    ]
                ),
            ),
        ]
    )
    llms_doc = "\n".join(
        [
            f"Contextro local docs bundle for project {root.name}.",
            "Read order:",
            "1. index.md - bundle overview, entry points, and command hints",
            "2. architecture.md - layers, entry points, and highest-blast-radius files",
            "3. audit.md - prioritized risks, dead code, cycles, and coverage gaps",
            "4. llms.txt - this terse summary",
            "",
            f"Project root: {root}",
            f"Files indexed: {len(repo_map.modules)}",
            f"Entry points: {', '.join(restore['entry_points'][:10]) or '(none)'}",
            (
                "Top blast-radius files: "
                + ", ".join(item["path"] for item in audit["blast_radius_hotspots"][:8])
                if audit["blast_radius_hotspots"]
                else "Top blast-radius files: (none)"
            ),
            "Priority actions:",
            _llms_recommendations(audit["recommendation_details"]),
            "",
            "Suggested commands:",
            "- contextro restore",
            "- contextro focus <file>",
            "- contextro audit",
        ]
    ).strip()
    return {
        "index.md": index_doc,
        "architecture.md": architecture_doc,
        "audit.md": audit_doc,
        "llms.txt": llms_doc,
    }


def _resolve_relative_path(root: Path, target_path: str) -> str:
    candidate = Path(target_path).expanduser()
    if not candidate.is_absolute():
        candidate = (root / candidate).resolve()
    else:
        candidate = candidate.resolve()
    return candidate.relative_to(root).as_posix()


def _require_root(state) -> Path:
    root = getattr(state, "codebase_path", None)
    if root is None:
        raise ValueError("No codebase indexed. Run index first.")
    return Path(root).resolve()


def _safe_read(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return path.read_text(encoding="utf-8", errors="replace")


def _section(title: str, body: str, *, level: int = 2) -> str:
    heading = "#" * min(level, 6)
    return f"{heading} {title}\n{body.strip()}".strip()


def _code_list(values: list[str] | tuple[str, ...]) -> str:
    if not values:
        return "- (none)"
    return "\n".join(f"- `{value}`" for value in values)


def _mapping_bullets(values: dict[str, Any]) -> str:
    if not values:
        return "- (none)"
    return "\n".join(f"- **{key}:** {value}" for key, value in values.items())


def _ordered_summary_bullets(data: dict[str, Any], keys: tuple[str, ...]) -> str:
    lines = []
    for key in keys:
        if key not in data:
            continue
        lines.append(f"- **{key.replace('_', ' ').title()}:** {data[key]}")
    return "\n".join(lines) if lines else "- (none)"


def _hub_items_markdown(items: list[dict[str, Any]]) -> str:
    if not items:
        return "- (none)"
    return "\n".join(
        (
            f"- `{item['path']}` — degree {item['degree']} "
            f"(imports {item['imports']}, dependents {item['dependents']}, "
            f"calls {item['calls']}, called by {item['called_by']})"
        )
        for item in items
    )


def _complexity_items_markdown(items: list[dict[str, Any]]) -> str:
    if not items:
        return "- (none)"
    return "\n".join(
        f"- `{item['name']}` at `{item['location']}` — complexity {item['complexity']}"
        for item in items
    )


def _unused_symbol_markdown(items: list[dict[str, Any]]) -> str:
    if not items:
        return "- (none)"
    return "\n".join(
        (
            f"- `{item['name']}` at `{item['location']}` — {item['reason']}"
            if "name" in item
            else f"- `{item['location']}` — {item['reason']}"
        )
        for item in items
    )


def _cycles_markdown(cycles: list[dict[str, Any]]) -> str:
    if not cycles:
        return "- (none)"
    return "\n".join(
        (
            f"- `{ ' → '.join(cycle['cycle']) }` "
            f"({cycle['kind'].replace('_', ' ')}, {cycle['length']} file(s))"
        )
        for cycle in cycles
    )


def _cycle_files(cycles: list[dict[str, Any]], *, limit: int = 6) -> list[str]:
    files = []
    seen = set()
    for cycle in cycles:
        for path in cycle["files"]:
            if path in seen:
                continue
            seen.add(path)
            files.append(path)
            if len(files) >= limit:
                return files
    return files


def _recommendation_details_markdown(items: list[dict[str, Any]]) -> str:
    lines = []
    for index, item in enumerate(items, start=1):
        lines.append(
            f"{index}. **{item['priority'].title()} · "
            f"{item['category'].replace('_', ' ').title()}** — {item['action']}"
        )
        lines.append(f"   - Reason: {item['reason']}")
        evidence = _evidence_summary(item.get("evidence", {}))
        if evidence:
            lines.append(f"   - Evidence: {evidence}")
    return "\n".join(lines) if lines else "- (none)"


def _evidence_summary(evidence: dict[str, Any]) -> str:
    parts = []
    for key, value in evidence.items():
        label = key.replace("_", " ")
        if isinstance(value, list):
            if not value:
                continue
            rendered = ", ".join(f"`{item}`" for item in value)
            parts.append(f"{label}: {rendered}")
        else:
            parts.append(f"{label}: {value}")
    return "; ".join(parts)


def _audit_summary_markdown(summary: dict[str, Any]) -> str:
    return _ordered_summary_bullets(
        summary,
        (
            "quality_score",
            "maintainability_index",
            "coverage_ratio",
            "dead_files",
            "dead_symbols",
            "circular_dependencies",
            "coverage_gaps",
            "high_complexity_functions",
            "uncovered_hub_files",
        ),
    )


def _architecture_snapshot_markdown(restore: dict[str, Any]) -> str:
    return "\n\n".join(
        [
            "### Layers\n" + _mapping_bullets(restore["layers"]),
            "### Hub Files\n" + _hub_items_markdown(restore["hub_files"]),
        ]
    )


def _recent_session_markdown(recent_session: dict[str, Any], archive: dict[str, Any]) -> str:
    parts = []
    snapshot_lines = _ordered_summary_bullets(
        recent_session,
        ("session_duration_min", "total_events"),
    )
    if snapshot_lines != "- (none)":
        parts.append(snapshot_lines)
    key_actions = recent_session.get("key_actions", [])
    if key_actions:
        parts.append("### Key Actions\n" + _code_list(key_actions[:8]))
    archive_entries = archive.get("entries")
    if archive_entries:
        parts.append(f"- **Archived Context Entries:** {archive_entries}")
    return "\n\n".join(parts) if parts else "- (none)"


def _llms_recommendations(items: list[dict[str, Any]]) -> str:
    if not items:
        return "- none"
    return "\n".join(
        f"- {item['priority']}: {item['action']}" for item in items[:5]
    )
