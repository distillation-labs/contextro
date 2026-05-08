"""Dedicated static analyses built on top of the repository map."""

from __future__ import annotations

from collections import defaultdict
from pathlib import Path
from typing import Any

from contextro_mcp.analysis.repository_map import (
    RepositoryMap,
    is_private_symbol,
    looks_like_entry_symbol,
    reachable_paths,
)
from contextro_mcp.core.graph_models import NodeType, RelationshipType
from contextro_mcp.engines.graph_engine import RustworkxCodeGraph


def analyze_dead_code(
    repo_map: RepositoryMap,
    graph: RustworkxCodeGraph,
    *,
    limit: int = 200,
) -> dict[str, Any]:
    """Return a conservative, graph-based dead code report."""
    production_entry_points = [
        path
        for path in repo_map.entry_points
        if (module := repo_map.modules.get(path)) is not None and not module.is_test
    ]
    reachable = reachable_paths(repo_map, production_entry_points, relation="combined")

    unused_files = sorted(
        path
        for path, module in repo_map.modules.items()
        if not module.is_test and path not in reachable
    )
    called_symbol_ids = set()
    for rel in graph.get_relationships_by_type(RelationshipType.CALLS):
        source = graph.get_node(rel.source_id)
        if source is None:
            continue
        source_path = _node_relative_path(source.location.file_path, repo_map.root_path)
        if source_path in reachable:
            called_symbol_ids.add(rel.target_id)

    unused_symbols: list[dict[str, Any]] = []
    for path, module in repo_map.modules.items():
        if module.is_test:
            continue
        file_unreachable = path in unused_files
        imported_by_others = bool(module.dependents)
        for symbol_id in module.symbol_ids:
            node = graph.get_node(symbol_id)
            if node is None:
                continue
            if node.node_type not in {NodeType.FUNCTION, NodeType.CLASS, NodeType.VARIABLE}:
                continue
            if looks_like_entry_symbol(node.name):
                continue
            if file_unreachable:
                unused_symbols.append(
                    {
                        "name": node.name,
                        "location": f"{path}:{node.location.start_line}",
                        "reason": "File is unreachable from production entry points",
                        "confidence": "high",
                    }
                )
                continue
            if symbol_id in called_symbol_ids:
                continue
            if imported_by_others and not is_private_symbol(node.name):
                continue
            if node.name.startswith("__") and node.name.endswith("__"):
                continue
            if not is_private_symbol(node.name):
                continue
            unused_symbols.append(
                {
                    "name": node.name,
                    "location": f"{path}:{node.location.start_line}",
                    "reason": "Private symbol has no callers",
                    "confidence": "medium",
                }
            )

    unresolved_imports = [
        {"source": item.source, "specifier": item.specifier}
        for item in repo_map.unresolved_imports[:limit]
    ]
    return {
        "entry_points": production_entry_points,
        "reachable_files": sorted(reachable)[:limit],
        "unused_files": unused_files[:limit],
        "unused_symbols": unused_symbols[:limit],
        "unresolved_imports": unresolved_imports,
        "summary": {
            "production_entry_points": len(production_entry_points),
            "reachable_files": len(reachable),
            "unused_files": len(unused_files),
            "unused_symbols": len(unused_symbols),
            "unresolved_imports": len(repo_map.unresolved_imports),
        },
    }


def analyze_circular_dependencies(
    repo_map: RepositoryMap,
    *,
    limit: int = 100,
) -> dict[str, Any]:
    """Return Tarjan SCC-based circular dependency findings at the file level."""
    adjacency = {path: set(module.imports) for path, module in repo_map.modules.items()}
    components = _tarjan_components(adjacency)
    cycles = []
    for component in components:
        cycle = _short_cycle(component, adjacency)
        cycles.append(
            {
                "files": component,
                "length": len(component),
                "cycle": cycle if cycle else component,
                "kind": "self_cycle" if len(component) == 1 else "multi_file_cycle",
            }
        )

    return {
        "summary": {
            "cycle_count": len(cycles),
            "files_in_cycles": len({item for cycle in cycles for item in cycle["files"]}),
            "self_cycles": sum(1 for cycle in cycles if cycle["kind"] == "self_cycle"),
        },
        "cycles": cycles[:limit],
    }


def analyze_test_coverage_map(
    repo_map: RepositoryMap,
    graph: RustworkxCodeGraph,
    *,
    limit: int = 200,
) -> dict[str, Any]:
    """Return a graph-based static test coverage map."""
    production_files = sorted(
        path for path, module in repo_map.modules.items() if not module.is_test
    )
    tests_by_file = _tests_by_production_file(repo_map)
    file_coverage = [
        {"path": path, "covered": bool(tests_by_file[path]), "tests": list(tests_by_file[path])}
        for path in production_files
    ]
    uncovered_files = [item["path"] for item in file_coverage if not item["covered"]]

    symbol_reachable = _reachable_symbol_ids(repo_map, graph)
    uncovered_symbols: list[dict[str, Any]] = []
    for path, module in repo_map.modules.items():
        if module.is_test:
            continue
        tests = tests_by_file.get(path, ())
        for symbol_id in module.symbol_ids:
            node = graph.get_node(symbol_id)
            if node is None or node.node_type not in {NodeType.FUNCTION, NodeType.CLASS}:
                continue
            if symbol_id in symbol_reachable:
                continue
            if looks_like_entry_symbol(node.name):
                continue
            uncovered_symbols.append(
                {
                    "name": node.name,
                    "location": f"{path}:{node.location.start_line}",
                    "reason": (
                        "Tests reach the file, but no static call path reaches this symbol"
                        if tests
                        else "No transitive test path found"
                    ),
                }
            )

    coverage_ratio = 0.0
    if production_files:
        coverage_ratio = (
            (len(production_files) - len(uncovered_files)) / len(production_files)
        ) * 100

    return {
        "summary": {
            "test_files": len(repo_map.test_files),
            "production_files": len(production_files),
            "covered_files": len(production_files) - len(uncovered_files),
            "uncovered_files": len(uncovered_files),
            "coverage_ratio": round(coverage_ratio, 2),
        },
        "file_coverage": file_coverage[:limit],
        "uncovered_files": uncovered_files[:limit],
        "uncovered_symbols": uncovered_symbols[:limit],
        "note": "Static reachability only; this is not runtime coverage.",
    }


def _short_cycle(component: list[str], adjacency: dict[str, set[str]]) -> list[str]:
    component_set = set(component)
    for start in component:
        cycle = _find_cycle(start, start, adjacency, component_set, [start], {start})
        if cycle:
            return cycle
    return component


def _reachable_symbol_ids(repo_map: RepositoryMap, graph: RustworkxCodeGraph) -> set[str]:
    seed_ids = {
        node.id
        for node in graph.nodes.values()
        if _node_relative_path(node.location.file_path, repo_map.root_path) in repo_map.test_files
    }
    if not seed_ids:
        return set()
    return {
        node.id
        for node in graph.get_reachable_nodes(
            sorted(seed_ids),
            relationship_types={RelationshipType.CALLS},
        )
    }


def _tests_by_production_file(repo_map: RepositoryMap) -> dict[str, tuple[str, ...]]:
    tests_by_file: dict[str, set[str]] = defaultdict(set)
    for test_file in repo_map.test_files:
        for reachable_path in reachable_paths(repo_map, [test_file], relation="combined"):
            module = repo_map.modules.get(reachable_path)
            if module and not module.is_test:
                tests_by_file[reachable_path].add(test_file)
    return {
        path: tuple(sorted(tests_by_file.get(path, ())))
        for path, module in repo_map.modules.items()
        if not module.is_test
    }


def _tarjan_components(adjacency: dict[str, set[str]]) -> list[list[str]]:
    index = 0
    indices: dict[str, int] = {}
    lowlinks: dict[str, int] = {}
    stack: list[str] = []
    on_stack: set[str] = set()
    components: list[list[str]] = []

    def strongconnect(node: str) -> None:
        nonlocal index
        indices[node] = index
        lowlinks[node] = index
        index += 1
        stack.append(node)
        on_stack.add(node)

        for neighbor in sorted(adjacency.get(node, ())):
            if neighbor not in indices:
                strongconnect(neighbor)
                lowlinks[node] = min(lowlinks[node], lowlinks[neighbor])
            elif neighbor in on_stack:
                lowlinks[node] = min(lowlinks[node], indices[neighbor])

        if lowlinks[node] == indices[node]:
            component: list[str] = []
            while stack:
                member = stack.pop()
                on_stack.discard(member)
                component.append(member)
                if member == node:
                    break
            has_self_loop = len(component) == 1 and node in adjacency.get(node, set())
            if len(component) > 1 or has_self_loop:
                components.append(sorted(component))

    for node in sorted(adjacency):
        if node not in indices:
            strongconnect(node)
    return sorted(components, key=lambda item: (-len(item), item))


def _find_cycle(
    start: str,
    current: str,
    adjacency: dict[str, set[str]],
    component: set[str],
    trail: list[str],
    seen: set[str],
) -> list[str] | None:
    for neighbor in sorted(adjacency.get(current, ())):
        if neighbor not in component:
            continue
        if neighbor == start:
            return trail + [start]
        if neighbor in seen:
            continue
        cycle = _find_cycle(
            start,
            neighbor,
            adjacency,
            component,
            trail + [neighbor],
            seen | {neighbor},
        )
        if cycle:
            return cycle
    return None


def _node_relative_path(file_path: str, root_path: str) -> str | None:
    try:
        return Path(file_path).resolve().relative_to(root_path).as_posix()
    except ValueError:
        return None
