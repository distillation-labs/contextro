"""Deterministic repository map built from local files plus the indexed graph.

This layer fills the gap between Contextro's symbol graph and file/module-oriented
product surfaces such as sidecars, focus, restore, and static analyses.
"""

from __future__ import annotations

import json
import os
import re
import shlex
from collections import defaultdict, deque
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from contextro_mcp.config import Settings, get_settings
from contextro_mcp.core.graph_models import NodeType, RelationshipType
from contextro_mcp.engines.graph_engine import RustworkxCodeGraph
from contextro_mcp.indexing.file_discovery import SKIP_DIRS, discover_files

_PYTHON_SUFFIXES = {".py"}
_JS_SUFFIXES = {".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx"}
_ALL_SUFFIXES = tuple(sorted(_PYTHON_SUFFIXES | _JS_SUFFIXES))

_PY_IMPORT_RE = re.compile(r"^\s*import\s+([A-Za-z0-9_., ]+)", re.MULTILINE)
_PY_FROM_RE = re.compile(
    r"^\s*from\s+([.A-Za-z0-9_]+)\s+import\s+([A-Za-z0-9_.*, ()]+)",
    re.MULTILINE,
)
_JS_IMPORT_RE = re.compile(
    r"""(?:import|export)\s+(?:type\s+)?(?:[\s\w{},*$]+\s+from\s+)?["']([^"']+)["']"""
)
_JS_REQUIRE_RE = re.compile(r"""require\(\s*["']([^"']+)["']\s*\)""")
_JS_DYNAMIC_IMPORT_RE = re.compile(r"""import\(\s*["']([^"']+)["']\s*\)""")
_PYPROJECT_SECTION_RE = re.compile(r"^\s*\[([^\]]+)\]\s*$")
_PYPROJECT_ASSIGNMENT_RE = re.compile(r"""^\s*([A-Za-z0-9_.-]+)\s*=\s*["']([^"']+)["']""")

_ENTRY_FILENAMES = {
    "main.py",
    "__main__.py",
    "app.py",
    "cli.py",
    "server.py",
    "manage.py",
    "wsgi.py",
    "asgi.py",
    "main.ts",
    "main.tsx",
    "main.js",
    "main.jsx",
    "index.ts",
    "index.tsx",
    "index.js",
    "index.jsx",
    "cli.ts",
    "cli.tsx",
    "cli.js",
    "cli.jsx",
    "server.ts",
    "server.tsx",
    "server.js",
    "server.jsx",
}
_ENTRY_DIRS = {"bin", "scripts"}
_PACKAGE_SCRIPT_ENTRY_NAMES = {
    "start",
    "dev",
    "serve",
    "preview",
    "cli",
    "worker",
    "daemon",
    "api",
    "web",
    "app",
}
_SCRIPT_EXECUTABLES = {
    "node",
    "bun",
    "deno",
    "tsx",
    "ts-node",
    "vite-node",
    "babel-node",
    "python",
    "python3",
}
_SCRIPT_SUBCOMMANDS = {"run", "exec", "watch"}
_SCRIPT_OPTION_TOKENS = {"-r", "--require", "--loader", "--import"}
_SCRIPT_SPLIT_RE = re.compile(r"\s*(?:&&|\|\||;)\s*")

_PRIVATE_SYMBOL_PREFIXES = ("_",)
_ENTRY_SYMBOL_NAMES = {
    "main",
    "__main__",
    "run",
    "start",
    "setup",
    "__init__",
    "handler",
    "callback",
}


@dataclass(frozen=True, slots=True)
class ImportLink:
    """Resolved or unresolved local import."""

    source: str
    specifier: str
    target: str | None


@dataclass(frozen=True, slots=True)
class ModuleSummary:
    """File-level summary used by product surfaces and static analyses."""

    path: str
    language: str
    imports: tuple[str, ...]
    dependents: tuple[str, ...]
    calls: tuple[str, ...]
    called_by: tuple[str, ...]
    symbol_ids: tuple[str, ...]
    symbol_names: tuple[str, ...]
    is_test: bool
    is_entry: bool


@dataclass(frozen=True, slots=True)
class RepositoryMap:
    """Local-first, file-oriented map of the repository."""

    root_path: str
    modules: dict[str, ModuleSummary]
    entry_points: tuple[str, ...]
    test_files: tuple[str, ...]
    unresolved_imports: tuple[ImportLink, ...]

    def neighbors(
        self,
        path: str,
        *,
        relation: str = "imports",
        reverse: bool = False,
    ) -> tuple[str, ...]:
        """Return adjacent file paths for the requested relationship kind."""
        module = self.modules.get(path)
        if module is None:
            return ()
        if relation == "imports":
            return module.dependents if reverse else module.imports
        if relation == "calls":
            return module.called_by if reverse else module.calls
        if relation == "combined":
            values = set(module.imports if not reverse else module.dependents)
            values.update(module.calls if not reverse else module.called_by)
            return tuple(sorted(values))
        raise ValueError(f"Unsupported relation kind: {relation}")


def build_repository_map(
    root_path: Path,
    graph: RustworkxCodeGraph,
    settings: Settings | None = None,
) -> RepositoryMap:
    """Build a file-level repository map from the current codebase and graph."""
    root_path = root_path.resolve()
    settings = settings or get_settings()
    files = [path.resolve() for path in discover_files(root_path, settings)]
    relative_paths = {
        path: path.relative_to(root_path).as_posix() for path in files if path.is_file()
    }
    rel_to_abs = {rel: path for path, rel in relative_paths.items()}

    module_index = _build_python_module_index(relative_paths)
    local_roots = {Path(rel_path).parts[0] for rel_path in rel_to_abs}
    imports_by_file: dict[str, set[str]] = defaultdict(set)
    unresolved_imports: list[ImportLink] = []

    for abs_path, rel_path in relative_paths.items():
        text = _safe_read_text(abs_path)
        specifiers = _extract_import_specifiers(abs_path, text)
        for specifier in specifiers:
            target = _resolve_import(specifier, rel_path, rel_to_abs, module_index)
            if target is None:
                if _looks_like_local_import(specifier, module_index, local_roots):
                    unresolved_imports.append(
                        ImportLink(source=rel_path, specifier=specifier, target=None)
                    )
                continue
            imports_by_file[rel_path].add(target)

    dependents_by_file: dict[str, set[str]] = defaultdict(set)
    for source, targets in imports_by_file.items():
        for target in targets:
            dependents_by_file[target].add(source)

    symbols_by_file: dict[str, list[str]] = defaultdict(list)
    symbol_names_by_file: dict[str, list[str]] = defaultdict(list)
    calls_by_file: dict[str, set[str]] = defaultdict(set)
    called_by_file: dict[str, set[str]] = defaultdict(set)
    for node in graph.nodes.values():
        try:
            rel_path = str(Path(node.location.file_path).resolve().relative_to(root_path))
        except ValueError:
            continue
        rel_path = rel_path.replace("\\", "/")
        if node.node_type != NodeType.MODULE:
            symbols_by_file[rel_path].append(node.id)
            symbol_names_by_file[rel_path].append(node.name)

    for rel in graph.get_relationships_by_type(RelationshipType.CALLS):
        source = graph.get_node(rel.source_id)
        target = graph.get_node(rel.target_id)
        if source is None or target is None:
            continue
        try:
            source_rel = str(Path(source.location.file_path).resolve().relative_to(root_path))
            target_rel = str(Path(target.location.file_path).resolve().relative_to(root_path))
        except ValueError:
            continue
        source_rel = source_rel.replace("\\", "/")
        target_rel = target_rel.replace("\\", "/")
        if source_rel == target_rel:
            continue
        calls_by_file[source_rel].add(target_rel)
        called_by_file[target_rel].add(source_rel)

    file_kinds = {
        rel_path: {
            "language": _language_for_path(rel_path),
            "is_test": _is_test_file(rel_path),
        }
        for rel_path in rel_to_abs
    }
    entry_points = _detect_entry_points(
        root_path,
        rel_to_abs,
        imports_by_file,
        file_kinds,
        module_index,
    )

    modules = {
        rel_path: ModuleSummary(
            path=rel_path,
            language=file_kinds[rel_path]["language"],
            imports=tuple(sorted(imports_by_file.get(rel_path, ()))),
            dependents=tuple(sorted(dependents_by_file.get(rel_path, ()))),
            calls=tuple(sorted(calls_by_file.get(rel_path, ()))),
            called_by=tuple(sorted(called_by_file.get(rel_path, ()))),
            symbol_ids=tuple(sorted(symbols_by_file.get(rel_path, ()))),
            symbol_names=tuple(sorted(set(symbol_names_by_file.get(rel_path, ())))),
            is_test=file_kinds[rel_path]["is_test"],
            is_entry=rel_path in entry_points,
        )
        for rel_path in sorted(rel_to_abs)
    }
    test_files = tuple(sorted(path for path, meta in file_kinds.items() if meta["is_test"]))

    return RepositoryMap(
        root_path=str(root_path),
        modules=modules,
        entry_points=tuple(sorted(entry_points)),
        test_files=test_files,
        unresolved_imports=tuple(unresolved_imports),
    )


def reachable_paths(
    repo_map: RepositoryMap,
    starts: Iterable[str],
    *,
    relation: str = "imports",
    reverse: bool = False,
) -> set[str]:
    """Return the transitive closure of files reachable from the given starts."""
    queue = deque(start for start in starts if start in repo_map.modules)
    seen: set[str] = set()
    while queue:
        current = queue.popleft()
        if current in seen:
            continue
        seen.add(current)
        for neighbor in repo_map.neighbors(current, relation=relation, reverse=reverse):
            if neighbor not in seen:
                queue.append(neighbor)
    return seen


def related_tests(repo_map: RepositoryMap, path: str) -> tuple[str, ...]:
    """Return test files that can reach the target file via imports or calls."""
    tests = [
        test_file
        for test_file in repo_map.test_files
        if path in reachable_paths(repo_map, [test_file], relation="combined")
    ]
    return tuple(sorted(tests))


def top_degree_files(repo_map: RepositoryMap, limit: int = 10) -> list[dict[str, int | str]]:
    """Return the most connected files in the repository."""
    ranked = []
    for path, module in repo_map.modules.items():
        degree = len(
            set(module.imports)
            | set(module.dependents)
            | set(module.calls)
            | set(module.called_by)
        )
        ranked.append(
            {
                "path": path,
                "degree": degree,
                "imports": len(module.imports),
                "dependents": len(module.dependents),
                "calls": len(module.calls),
                "called_by": len(module.called_by),
            }
        )
    ranked.sort(
        key=lambda item: (item["degree"], item["dependents"], item["called_by"]),
        reverse=True,
    )
    return ranked[:limit]


def layer_hint(path: str) -> str:
    """Guess a lightweight architecture/layer hint from the file path."""
    parts = Path(path).parts
    lowered = [part.lower() for part in parts]
    if any(part in {"tests", "test", "__tests__", "spec"} for part in lowered):
        return "test"
    if any(part in {"cli", "commands", "bin", "scripts"} for part in lowered):
        return "cli"
    if any(part in {"api", "routes", "handlers", "controllers"} for part in lowered):
        return "api"
    if any(part in {"ui", "components", "pages", "app"} for part in lowered):
        return "ui"
    if any(part in {"models", "schema", "entities"} for part in lowered):
        return "model"
    if any(part in {"core", "engine", "execution", "services"} for part in lowered):
        return "core"
    if len(parts) > 1:
        return parts[0]
    return "root"


def is_private_symbol(name: str) -> bool:
    """Return whether a symbol looks intentionally private."""
    return name.startswith(_PRIVATE_SYMBOL_PREFIXES) and not name.startswith("__")


def looks_like_entry_symbol(name: str) -> bool:
    """Return whether a symbol name looks like an entry point hook."""
    lowered = name.lower()
    return lowered in _ENTRY_SYMBOL_NAMES or lowered.startswith("test_")


def _safe_read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return path.read_text(encoding="utf-8", errors="replace")


def _extract_import_specifiers(path: Path, text: str) -> tuple[str, ...]:
    suffix = path.suffix.lower()
    if suffix in _PYTHON_SUFFIXES:
        return _extract_python_imports(text)
    if suffix in _JS_SUFFIXES:
        return _extract_js_imports(text)
    return ()


def _extract_python_imports(text: str) -> tuple[str, ...]:
    specifiers: list[str] = []
    for match in _PY_IMPORT_RE.finditer(text):
        raw = match.group(1)
        for item in raw.split(","):
            name = item.strip().split(" as ", 1)[0].strip()
            if name:
                specifiers.append(name)
    for match in _PY_FROM_RE.finditer(text):
        base = match.group(1).strip()
        imported = match.group(2)
        if base:
            specifiers.append(base)
        for item in imported.replace("(", "").replace(")", "").split(","):
            name = item.strip().split(" as ", 1)[0].strip()
            if not name or name == "*":
                continue
            specifiers.append(f"{base}.{name}" if base else name)
    return tuple(dict.fromkeys(specifiers))


def _extract_js_imports(text: str) -> tuple[str, ...]:
    specifiers = [
        *[match.group(1).strip() for match in _JS_IMPORT_RE.finditer(text)],
        *[match.group(1).strip() for match in _JS_REQUIRE_RE.finditer(text)],
        *[match.group(1).strip() for match in _JS_DYNAMIC_IMPORT_RE.finditer(text)],
    ]
    return tuple(dict.fromkeys(specifiers))


def _build_python_module_index(relative_paths: dict[Path, str]) -> dict[str, list[str]]:
    index: dict[str, list[str]] = defaultdict(list)
    for rel_path in relative_paths.values():
        path = Path(rel_path)
        if path.suffix.lower() not in _PYTHON_SUFFIXES:
            continue
        module_parts = list(path.with_suffix("").parts)
        if module_parts and module_parts[-1] == "__init__":
            module_parts = module_parts[:-1]
        if not module_parts:
            continue
        for start in range(len(module_parts)):
            candidate = ".".join(module_parts[start:])
            if candidate:
                index[candidate].append(rel_path)
    return index


def _resolve_import(
    specifier: str,
    source_rel: str,
    rel_to_abs: dict[str, Path],
    module_index: dict[str, list[str]],
) -> str | None:
    source_path = Path(source_rel)
    suffix = source_path.suffix.lower()
    if suffix in _PYTHON_SUFFIXES:
        return _resolve_python_import(specifier, source_rel, module_index)
    if suffix in _JS_SUFFIXES:
        return _resolve_js_import(specifier, source_rel, rel_to_abs)
    return None


def _resolve_python_import(
    specifier: str,
    source_rel: str,
    module_index: dict[str, list[str]],
) -> str | None:
    dots = len(specifier) - len(specifier.lstrip("."))
    import_name = specifier.lstrip(".")
    source_module = list(Path(source_rel).with_suffix("").parts)
    if source_module and source_module[-1] == "__init__":
        current_package = source_module[:-1]
    else:
        current_package = source_module[:-1]

    if dots:
        trim = max(dots - 1, 0)
        base = current_package[: len(current_package) - trim]
        full_name = ".".join(part for part in [*base, *import_name.split(".")] if part)
        if not full_name:
            return None
        return _resolve_python_module_name(full_name, source_rel, module_index)

    return _resolve_python_module_name(import_name, source_rel, module_index)


def _resolve_python_module_name(
    module_name: str,
    source_rel: str,
    module_index: dict[str, list[str]],
) -> str | None:
    candidate = module_name
    while candidate:
        matches = module_index.get(candidate, [])
        if len(matches) == 1:
            return matches[0]
        if matches:
            return _pick_best_module_match(candidate, matches, source_rel)
        if "." not in candidate:
            break
        candidate = candidate.rsplit(".", 1)[0]
    return None


def _pick_best_module_match(module_name: str, matches: list[str], source_rel: str) -> str | None:
    if not matches:
        return None
    source_parts = Path(source_rel).parts
    ranked = sorted(
        matches,
        key=lambda item: (
            -_shared_prefix_length(source_parts, Path(item).parts),
            len(Path(item).parts),
            item,
        ),
    )
    return ranked[0]


def _shared_prefix_length(left: Iterable[str], right: Iterable[str]) -> int:
    count = 0
    for l_item, r_item in zip(left, right):
        if l_item != r_item:
            break
        count += 1
    return count


def _resolve_js_import(
    specifier: str,
    source_rel: str,
    rel_to_abs: dict[str, Path],
) -> str | None:
    source_parent = Path(source_rel).parent
    candidates: list[Path] = []
    if specifier.startswith("."):
        base = (source_parent / specifier).as_posix()
        candidates.extend(_js_candidates(Path(base)))
    else:
        if specifier.startswith("@/"):
            # @/ alias: try root, src/, app/, and the source file's own package root
            alias_path = specifier[2:]
            candidates.extend(_js_candidates(Path(alias_path)))
            candidates.extend(_js_candidates(Path("src") / alias_path))
            candidates.extend(_js_candidates(Path("app") / alias_path))
            # Also try relative to the source file's package root (first path component)
            source_parts = Path(source_rel).parts
            if len(source_parts) > 1:
                pkg_root = source_parts[0]
                candidates.extend(_js_candidates(Path(pkg_root) / alias_path))
                candidates.extend(_js_candidates(Path(pkg_root) / "src" / alias_path))
        elif specifier.startswith("/"):
            candidates.extend(_js_candidates(Path(specifier.lstrip("/"))))
        else:
            candidates.extend(_js_candidates(Path(specifier)))
            candidates.extend(_js_candidates(Path("src") / specifier))

    known = set(rel_to_abs)
    for candidate in candidates:
        normalized = candidate.as_posix().lstrip("./")
        if normalized in known:
            return normalized
    return None


def _js_candidates(base: Path) -> list[Path]:
    candidates = [base]
    if base.suffix:
        return candidates
    for suffix in _ALL_SUFFIXES:
        candidates.append(base.with_suffix(suffix))
        candidates.append(base / f"index{suffix}")
    return candidates


def _entrypoint_candidates(base: Path) -> list[Path]:
    candidates = [base]
    if base.suffix:
        return candidates
    for suffix in _ALL_SUFFIXES:
        candidates.append(base.with_suffix(suffix))
        candidates.append(base / f"index{suffix}")
    return candidates


def _language_for_path(rel_path: str) -> str:
    suffix = Path(rel_path).suffix.lower()
    if suffix in _PYTHON_SUFFIXES:
        return "python"
    if suffix in {".ts", ".tsx"}:
        return "typescript"
    if suffix in {".js", ".jsx", ".mjs", ".cjs"}:
        return "javascript"
    return suffix.lstrip(".") or "text"


def _is_test_file(rel_path: str) -> bool:
    path = Path(rel_path)
    lowered_parts = [part.lower() for part in path.parts]
    lowered_name = path.name.lower()
    if any(part in {"tests", "test", "__tests__", "spec"} for part in lowered_parts):
        return True
    return (
        lowered_name.startswith("test_")
        or lowered_name.endswith("_test.py")
        or lowered_name.endswith(".test.ts")
        or lowered_name.endswith(".test.tsx")
        or lowered_name.endswith(".test.js")
        or lowered_name.endswith(".test.jsx")
        or lowered_name.endswith(".spec.ts")
        or lowered_name.endswith(".spec.tsx")
        or lowered_name.endswith(".spec.js")
        or lowered_name.endswith(".spec.jsx")
    )


def _detect_entry_points(
    root_path: Path,
    rel_to_abs: dict[str, Path],
    imports_by_file: dict[str, set[str]],
    file_kinds: dict[str, dict[str, str | bool]],
    module_index: dict[str, list[str]],
) -> set[str]:
    entry_points = set(_package_json_entry_points(root_path, rel_to_abs))
    entry_points.update(_package_json_script_entry_points(root_path, rel_to_abs))
    entry_points.update(_pyproject_entry_points(root_path, rel_to_abs, module_index))
    for rel_path in rel_to_abs:
        if file_kinds[rel_path]["is_test"]:
            continue
        path = Path(rel_path)
        if path.name in _ENTRY_FILENAMES or any(part in _ENTRY_DIRS for part in path.parts):
            entry_points.add(rel_path)
    if not entry_points:
        imported = {target for targets in imports_by_file.values() for target in targets}
        entry_points.update(
            rel_path
            for rel_path in rel_to_abs
            if not file_kinds[rel_path]["is_test"] and rel_path not in imported
        )
    if not entry_points:
        first = next(
            (
                rel_path
                for rel_path in sorted(rel_to_abs)
                if not file_kinds[rel_path]["is_test"]
            ),
            None,
        )
        if first:
            entry_points.add(first)
    return entry_points


def _package_json_entry_points(root_path: Path, rel_to_abs: dict[str, Path]) -> set[str]:
    entry_points: set[str] = set()
    known = set(rel_to_abs)
    for package_json in _metadata_files(root_path, "package.json"):
        try:
            data = json.loads(_safe_read_text(package_json))
        except json.JSONDecodeError:
            continue
        package_root = package_json.parent
        for candidate in _iter_package_json_entry_values(data):
            resolved = _resolve_package_json_entry(candidate, package_root, root_path)
            if resolved and resolved in known:
                entry_points.add(resolved)
    return entry_points


def _package_json_script_entry_points(root_path: Path, rel_to_abs: dict[str, Path]) -> set[str]:
    entry_points: set[str] = set()
    known = set(rel_to_abs)
    for package_json in _metadata_files(root_path, "package.json"):
        try:
            data = json.loads(_safe_read_text(package_json))
        except json.JSONDecodeError:
            continue
        scripts = data.get("scripts")
        if not isinstance(scripts, dict):
            continue
        package_root = package_json.parent
        for name, command in scripts.items():
            if not isinstance(command, str) or not _script_name_looks_entryish(name):
                continue
            for candidate in _iter_script_entry_candidates(command):
                resolved = _resolve_package_json_entry(candidate, package_root, root_path)
                if resolved and resolved in known and not _is_test_file(resolved):
                    entry_points.add(resolved)
    return entry_points


def _pyproject_entry_points(
    root_path: Path,
    rel_to_abs: dict[str, Path],
    module_index: dict[str, list[str]],
) -> set[str]:
    entry_points: set[str] = set()
    known = set(rel_to_abs)
    for pyproject in _metadata_files(root_path, "pyproject.toml"):
        for value in _iter_pyproject_entry_values(_safe_read_text(pyproject)):
            module_name = value.split(":", 1)[0].strip()
            if not module_name:
                continue
            resolved = _resolve_python_module_name(module_name, "", module_index)
            if resolved and resolved in known and not _is_test_file(resolved):
                entry_points.add(resolved)
    return entry_points


def _script_name_looks_entryish(name: str) -> bool:
    return name.lower().split(":", 1)[0] in _PACKAGE_SCRIPT_ENTRY_NAMES


def _iter_script_entry_candidates(command: str) -> Iterable[str]:
    for segment in _SCRIPT_SPLIT_RE.split(command):
        if not segment:
            continue
        try:
            tokens = shlex.split(segment, posix=True)
        except ValueError:
            tokens = segment.split()
        if not tokens:
            continue
        executable = Path(tokens[0]).name
        if executable not in _SCRIPT_EXECUTABLES:
            continue
        skip_value = False
        for token in tokens[1:]:
            normalized = token.strip().strip(",")
            if not normalized or normalized == "--":
                continue
            if skip_value:
                skip_value = False
                continue
            if normalized in _SCRIPT_OPTION_TOKENS:
                skip_value = True
                continue
            if normalized == "-m":
                break
            if normalized in _SCRIPT_SUBCOMMANDS:
                continue
            if normalized.startswith("-") or normalized.startswith("$"):
                continue
            if (
                "=" in normalized
                and "/" not in normalized
                and Path(normalized).suffix.lower() not in _ALL_SUFFIXES
            ):
                continue
            if _looks_like_entry_path_token(normalized):
                yield normalized.removeprefix("file:")
                break


def _looks_like_entry_path_token(token: str) -> bool:
    candidate = token.strip().strip("\"'")
    if not candidate or candidate.startswith(("http://", "https://")) or "*" in candidate:
        return False
    suffix = Path(candidate).suffix.lower()
    if suffix in _ALL_SUFFIXES:
        return True
    return candidate.startswith(("./", "../", "/")) or "/" in candidate


def _iter_pyproject_entry_values(text: str) -> Iterable[str]:
    current_section = ""
    for raw_line in text.splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        section_match = _PYPROJECT_SECTION_RE.match(line)
        if section_match:
            current_section = section_match.group(1).replace('"', "").replace("'", "")
            continue
        if not _section_contains_entry_points(current_section):
            continue
        value_match = _PYPROJECT_ASSIGNMENT_RE.match(line)
        if value_match:
            yield value_match.group(2).strip()


def _section_contains_entry_points(section: str) -> bool:
    return section in {
        "project.scripts",
        "project.gui-scripts",
        "tool.poetry.scripts",
    } or section.startswith("project.entry-points.")


def _iter_package_json_entry_values(data: dict) -> Iterable[str]:
    for key in ("main", "module", "browser", "types"):
        value = data.get(key)
        if isinstance(value, str):
            yield value

    bin_value = data.get("bin")
    if isinstance(bin_value, str):
        yield bin_value
    elif isinstance(bin_value, dict):
        for value in bin_value.values():
            if isinstance(value, str):
                yield value

    exports = data.get("exports")
    yield from _iter_export_values(exports)


def _iter_export_values(value: object) -> Iterable[str]:
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for nested in value.values():
            yield from _iter_export_values(nested)
    elif isinstance(value, list):
        for nested in value:
            yield from _iter_export_values(nested)


def _resolve_package_json_entry(candidate: str, package_root: Path, root_path: Path) -> str | None:
    if candidate.startswith("@/"):
        base = (root_path / candidate[2:]).resolve()
    elif candidate.startswith("/"):
        base = (root_path / candidate.lstrip("/")).resolve()
    else:
        base = (package_root / candidate).resolve()
    relative_base = base.relative_to(root_path) if base.is_relative_to(root_path) else base
    candidates = _entrypoint_candidates(relative_base)
    for item in candidates:
        absolute = (root_path / item).resolve() if not item.is_absolute() else item
        if absolute.exists():
            try:
                return absolute.relative_to(root_path).as_posix()
            except ValueError:
                continue
    return None


def _metadata_files(root_path: Path, filename: str) -> list[Path]:
    matches: list[Path] = []
    for dirpath, dirnames, filenames in os.walk(root_path):
        dirnames[:] = [
            name for name in dirnames if name not in SKIP_DIRS and not name.startswith(".")
        ]
        if filename in filenames:
            matches.append(Path(dirpath) / filename)
    return matches


def _looks_like_local_import(
    specifier: str,
    module_index: dict[str, list[str]],
    local_roots: set[str],
) -> bool:
    if specifier.startswith((".", "/", "@/")):
        return True
    head = specifier.split(".", 1)[0]
    return specifier in module_index or head in module_index or head in local_roots
