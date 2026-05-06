"""AST-grep based structural parser for code graph building.

Uses ast-grep for structural analysis across 25+ languages.
"""

import logging
from dataclasses import dataclass
from pathlib import Path
from typing import Dict

try:
    from ast_grep_py import SgRoot  # type: ignore[import-untyped]
except ImportError:
    SgRoot = None

from contextia_mcp.core.graph_models import (
    NodeType,
    RelationshipType,
    UniversalGraph,
    UniversalLocation,
    UniversalNode,
    UniversalRelationship,
)
from contextia_mcp.parsing.language_registry import (
    ASTGREP_LANGUAGES,
    get_language_for_file,
)

logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class AstGrepLanguageConfig:
    """Configuration for ast-grep language parsing."""

    name: str
    extensions: tuple
    ast_grep_id: str
    function_patterns: tuple
    class_patterns: tuple
    variable_patterns: tuple
    import_patterns: tuple


# ast-grep language ID mapping
ASTGREP_LANG_IDS: Dict[str, str] = {
    "python": "python",
    "javascript": "javascript",
    "typescript": "typescript",
    "c": "c",
    "cpp": "cpp",
    "go": "go",
    "java": "java",
    "kotlin": "kotlin",
    "rust": "rust",
    "ruby": "ruby",
    "php": "php",
    "swift": "swift",
    "csharp": "c_sharp",
    "scala": "scala",
    "lua": "lua",
    "dart": "dart",
}


class AstGrepParser:
    """Structural code parser using ast-grep for graph building."""

    def __init__(self):
        if SgRoot is None:
            raise ImportError(
                "ast-grep-py is required. Install with: pip install ast-grep-py"
            )
        self._node_counter = 0
        self._rel_counter = 0

    def can_parse(self, filepath: str) -> bool:
        lang = get_language_for_file(filepath)
        return lang is not None and lang in ASTGREP_LANGUAGES

    def parse_file(self, filepath: str, graph: UniversalGraph) -> int:
        """Parse a file and add nodes/relationships to the graph.

        Returns the number of nodes added.
        """
        file_path = Path(filepath)
        if not file_path.exists():
            raise FileNotFoundError(f"File not found: {filepath}")

        language = get_language_for_file(filepath)
        if language is None or language not in ASTGREP_LANGUAGES:
            return 0

        ast_grep_id = ASTGREP_LANG_IDS.get(language)
        if not ast_grep_id:
            return 0

        try:
            content = file_path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError) as e:
            logger.warning("Cannot read %s: %s", filepath, e)
            return 0

        # Create module node
        module_id = self._make_id("module")
        lines = content.count("\n") + 1
        module_node = UniversalNode(
            id=module_id,
            name=file_path.stem,
            node_type=NodeType.MODULE,
            location=UniversalLocation(
                file_path=str(filepath),
                start_line=1,
                end_line=lines,
                language=language,
            ),
            language=language,
            line_count=lines,
        )
        graph.add_node(module_node)
        nodes_added = 1

        try:
            root = SgRoot(content, ast_grep_id)
            sg_node = root.root()
        except Exception as e:
            logger.warning("ast-grep parse failed for %s: %s", filepath, e)
            return nodes_added

        # Extract functions
        nodes_added += self._extract_functions(
            sg_node, filepath, language, module_id, graph
        )
        # Extract classes
        nodes_added += self._extract_classes(
            sg_node, filepath, language, module_id, graph
        )
        # Extract imports
        self._extract_imports(sg_node, filepath, language, module_id, graph)

        # Extract function calls (builds CALLS edges)
        # Collect function names defined in this file for resolution
        func_nodes = {
            node.name: node.id
            for node in graph.nodes.values()
            if node.node_type == NodeType.FUNCTION
            and node.location.file_path == filepath
        }
        self._extract_calls(sg_node, filepath, language, module_id, graph, func_nodes)

        return nodes_added

    def parse_directory(
        self, directory: Path, graph: UniversalGraph, recursive: bool = True
    ) -> int:
        """Parse all supported files in a directory."""
        files_parsed = 0
        pattern = "**/*" if recursive else "*"

        for file_path in directory.glob(pattern):
            if not file_path.is_file():
                continue
            if not self.can_parse(str(file_path)):
                continue
            # Skip hidden/temp files
            if any(part.startswith(".") for part in file_path.parts):
                continue
            try:
                self.parse_file(str(file_path), graph)
                files_parsed += 1
            except Exception as e:
                logger.warning("Failed to parse %s: %s", file_path, e)

        return files_parsed

    def _make_id(self, prefix: str) -> str:
        self._node_counter += 1
        return f"{prefix}:{self._node_counter}"

    def _make_rel_id(self) -> str:
        self._rel_counter += 1
        return f"rel:{self._rel_counter}"

    def _extract_functions(
        self, sg_node, filepath: str, language: str, module_id: str,
        graph: UniversalGraph,
    ) -> int:
        """Extract function definitions from ast-grep node."""
        count = 0
        patterns = {
            "python": "def $NAME($$$PARAMS): $$$BODY",
            "javascript": "function $NAME($$$PARAMS) { $$$BODY }",
            "typescript": "function $NAME($$$PARAMS) { $$$BODY }",
            "go": "func $NAME($$$PARAMS) $$$BODY",
            "java": "$$$MODS $TYPE $NAME($$$PARAMS) { $$$BODY }",
            "rust": "fn $NAME($$$PARAMS) $$$BODY",
        }

        pattern = patterns.get(language)
        if not pattern:
            return 0

        try:
            matches = sg_node.find_all(pattern=pattern)
        except Exception as e:
            logger.debug("ast-grep find_all failed: %s", e)
            return 0

        for match in matches:
            try:
                rng = match.range()
                name_text = match.get_match("NAME")
                func_name = name_text.text() if name_text else f"anonymous_{self._node_counter}"

                func_id = self._make_id("func")
                func_node = UniversalNode(
                    id=func_id,
                    name=func_name,
                    node_type=NodeType.FUNCTION,
                    location=UniversalLocation(
                        file_path=filepath,
                        start_line=rng.start.line + 1,
                        end_line=rng.end.line + 1,
                        start_column=rng.start.column,
                        end_column=rng.end.column,
                        language=language,
                    ),
                    language=language,
                    line_count=rng.end.line - rng.start.line + 1,
                )
                graph.add_node(func_node)

                # Add CONTAINS relationship from module
                graph.add_relationship(UniversalRelationship(
                    id=self._make_rel_id(),
                    source_id=module_id,
                    target_id=func_id,
                    relationship_type=RelationshipType.CONTAINS,
                ))
                count += 1
            except Exception as e:
                logger.debug("Failed to extract function at %s: %s", filepath, e)
                continue

        return count

    def _extract_classes(
        self, sg_node, filepath: str, language: str, module_id: str,
        graph: UniversalGraph,
    ) -> int:
        """Extract class definitions from ast-grep node."""
        count = 0
        patterns = {
            "python": "class $NAME: $$$BODY",
            "javascript": "class $NAME { $$$BODY }",
            "typescript": "class $NAME { $$$BODY }",
            "java": "class $NAME { $$$BODY }",
            "rust": "struct $NAME { $$$BODY }",
        }

        pattern = patterns.get(language)
        if not pattern:
            return 0

        try:
            matches = sg_node.find_all(pattern=pattern)
        except Exception:
            return 0

        for match in matches:
            try:
                rng = match.range()
                name_text = match.get_match("NAME")
                class_name = name_text.text() if name_text else f"class_{self._node_counter}"

                class_id = self._make_id("class")
                class_node = UniversalNode(
                    id=class_id,
                    name=class_name,
                    node_type=NodeType.CLASS,
                    location=UniversalLocation(
                        file_path=filepath,
                        start_line=rng.start.line + 1,
                        end_line=rng.end.line + 1,
                        language=language,
                    ),
                    language=language,
                    line_count=rng.end.line - rng.start.line + 1,
                )
                graph.add_node(class_node)

                graph.add_relationship(UniversalRelationship(
                    id=self._make_rel_id(),
                    source_id=module_id,
                    target_id=class_id,
                    relationship_type=RelationshipType.CONTAINS,
                ))
                count += 1
            except Exception as e:
                logger.debug("Failed to extract class at %s: %s", filepath, e)
                continue

        return count

    def _extract_imports(
        self, sg_node, filepath: str, language: str, module_id: str,
        graph: UniversalGraph,
    ) -> None:
        """Extract import statements and add relationships."""
        patterns = {
            "python": ["import $NAME", "from $NAME import $$$ITEMS"],
            "javascript": ["import $$$ITEMS from '$NAME'"],
            "typescript": ["import $$$ITEMS from '$NAME'"],
            "go": ['import "$NAME"'],
            "java": ["import $NAME"],
            "rust": ["use $NAME"],
        }

        lang_patterns = patterns.get(language, [])
        for pattern in lang_patterns:
            try:
                matches = sg_node.find_all(pattern=pattern)
                for match in matches:
                    name_text = match.get_match("NAME")
                    if name_text:
                        import_name = name_text.text()
                        target_id = f"module:{import_name}"
                        graph.add_relationship(UniversalRelationship(
                            id=self._make_rel_id(),
                            source_id=module_id,
                            target_id=target_id,
                            relationship_type=RelationshipType.IMPORTS,
                        ))
            except Exception as e:
                logger.debug("Import match failed in %s: %s", filepath, e)
                continue

    def _extract_calls(
        self, sg_node, filepath: str, language: str, module_id: str,
        graph: UniversalGraph, func_nodes: dict,
    ) -> int:
        """Extract function call relationships.

        Finds all function calls and creates CALLS edges from the containing
        module to the called function name. Uses name-based resolution.

        Returns:
            Number of call relationships added.
        """
        count = 0

        # Patterns for function calls by language
        call_patterns = {
            "python": "$FUNC($$ARGS)",
            "javascript": "$FUNC($$ARGS)",
            "typescript": "$FUNC($$ARGS)",
            "go": "$FUNC($$ARGS)",
            "java": "$FUNC($$ARGS)",
            "rust": "$FUNC($$ARGS)",
            "ruby": "$FUNC($$ARGS)",
            "php": "$FUNC($$ARGS)",
        }

        pattern = call_patterns.get(language)
        if not pattern:
            return 0

        try:
            matches = sg_node.find_all(pattern=pattern)
        except Exception as e:
            logger.debug("Call extraction failed in %s: %s", filepath, e)
            return 0

        # Common builtins/keywords to skip
        skip_names = {
            "if", "for", "while", "return", "print", "len", "str", "int",
            "float", "bool", "list", "dict", "set", "tuple", "type", "super",
            "isinstance", "hasattr", "getattr", "setattr", "range", "enumerate",
            "map", "filter", "sorted", "reversed", "zip", "any", "all", "min",
            "max", "sum", "abs", "round", "open", "next", "iter", "format",
            "require", "console", "Math", "Object", "Array", "String", "Number",
            "Promise", "Error", "JSON", "Date", "RegExp", "Map", "Set",
            "setTimeout", "setInterval", "clearTimeout", "clearInterval",
            "parseInt", "parseFloat", "isNaN", "undefined",
        }

        seen_calls: set = set()
        for match in matches:
            try:
                func_text = match.get_match("FUNC")
                if not func_text:
                    continue
                called_name = func_text.text()
                if not called_name or len(called_name) > 100:
                    continue

                # Handle method calls: obj.method() -> extract "method"
                if "." in called_name:
                    parts = called_name.rsplit(".", 1)
                    called_name = parts[-1]

                # Skip builtins and keywords
                if called_name in skip_names:
                    continue
                # Skip names starting with uppercase that look like constructors/types
                # (keep them for classes though)
                if called_name[0].isupper() and called_name not in func_nodes:
                    continue

                # Dedup within file
                call_key = f"{module_id}->{called_name}"
                if call_key in seen_calls:
                    continue
                seen_calls.add(call_key)

                # Create CALLS relationship
                target_id = func_nodes.get(called_name, f"func:{called_name}")
                graph.add_relationship(UniversalRelationship(
                    id=self._make_rel_id(),
                    source_id=module_id,
                    target_id=target_id,
                    relationship_type=RelationshipType.CALLS,
                ))
                count += 1
            except Exception:
                continue

        return count
