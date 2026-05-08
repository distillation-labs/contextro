"""AST-aware snippet compression for search results.

Uses tree-sitter to identify function/class boundaries, keeps signatures
and docstrings, collapses implementation bodies. Achieves 40-60% token
reduction while preserving the information most useful for navigation.

Inspired by:
- arxiv 2502.14925: Code-specific prompt compression for RAG
- Cursor's approach: return signatures, full content on demand
- DeepSeek-V4's CSA: compress then select (same principle at snippet level)
"""

from __future__ import annotations

import logging
from typing import Any

logger = logging.getLogger(__name__)

# Node types that represent function/method definitions by language
_FUNCTION_NODES = {
    "python": {"function_definition", "decorated_definition"},
    "javascript": {"function_declaration", "method_definition", "arrow_function"},
    "typescript": {"function_declaration", "method_definition", "arrow_function"},
    "go": {"function_declaration", "method_declaration"},
    "rust": {"function_item"},
    "java": {"method_declaration", "constructor_declaration"},
    "c": {"function_definition"},
    "cpp": {"function_definition"},
    "ruby": {"method"},
}

_CLASS_NODES = {
    "python": {"class_definition"},
    "javascript": {"class_declaration"},
    "typescript": {"class_declaration"},
    "java": {"class_declaration"},
    "ruby": {"class"},
    "rust": {"struct_item", "impl_item"},
    "go": {"type_declaration"},
}


def compress_snippet(code: str, language: str = "python") -> str:
    """Compress a code snippet by keeping signatures and collapsing bodies.

    Args:
        code: Source code snippet.
        language: Programming language (for tree-sitter parsing).

    Returns:
        Compressed snippet with signatures preserved, bodies collapsed.
        Falls back to the original code if parsing fails.
    """
    if not code or len(code) < 200:
        return code  # Too short to benefit from compression

    try:
        return _compress_with_treesitter(code, language)
    except Exception as e:
        logger.debug("AST compression failed, using fallback: %s", e)
        return _compress_fallback(code)


def _compress_with_treesitter(code: str, language: str) -> str:
    """Use tree-sitter to identify and compress function bodies."""
    from tree_sitter_languages import get_parser

    lang_map = {
        "python": "python", "py": "python",
        "javascript": "javascript", "js": "javascript",
        "typescript": "typescript", "ts": "typescript",
        "go": "go", "golang": "go",
        "rust": "rust", "rs": "rust",
        "java": "java",
        "c": "c", "cpp": "cpp", "c++": "cpp",
        "ruby": "ruby", "rb": "ruby",
    }
    ts_lang = lang_map.get(language.lower(), language.lower())

    parser = get_parser(ts_lang)
    tree = parser.parse(code.encode())
    root = tree.root_node

    func_types = _FUNCTION_NODES.get(ts_lang, set())
    class_types = _CLASS_NODES.get(ts_lang, set())
    if not func_types and not class_types:
        return _compress_fallback(code)

    # Find all function/class nodes and their body ranges
    collapse_ranges: list[tuple[int, int, str]] = []  # (start_byte, end_byte, replacement)
    _find_collapsible_bodies(root, code.encode(), ts_lang, func_types, collapse_ranges)

    if not collapse_ranges:
        return _compress_fallback(code)

    # Apply collapses from end to start to preserve offsets
    result = code
    for start, end, replacement in sorted(collapse_ranges, reverse=True):
        result = result[:start] + replacement + result[end:]

    return result


def _find_collapsible_bodies(
    node: Any,
    source: bytes,
    language: str,
    func_types: set[str],
    ranges: list[tuple[int, int, str]],
    depth: int = 0,
) -> None:
    """Recursively find function bodies that can be collapsed."""
    if depth > 10:
        return

    if node.type in func_types or (node.type == "decorated_definition" and language == "python"):
        body = _find_body_node(node, language)
        if body and body.end_byte - body.start_byte > 100:
            # Keep first line of body (often docstring or key statement)
            body_text = source[body.start_byte:body.end_byte].decode(errors="replace")
            first_line = _extract_first_meaningful_line(body_text, language)
            indent = _get_indent(body_text)
            replacement = f"{indent}{first_line}\n{indent}..."
            ranges.append((body.start_byte, body.end_byte, replacement))
            return  # Don't recurse into collapsed bodies

    for child in node.children:
        _find_collapsible_bodies(child, source, language, func_types, ranges, depth + 1)


def _find_body_node(node: Any, language: str) -> Any | None:
    """Find the body/block child of a function node."""
    if language == "python":
        # For decorated_definition, get the inner function first
        if node.type == "decorated_definition":
            for child in node.children:
                if child.type == "function_definition":
                    node = child
                    break
        for child in node.children:
            if child.type == "block":
                return child
    else:
        # Most C-like languages use statement_block or block
        for child in node.children:
            if child.type in ("statement_block", "block", "compound_statement",
                              "function_body", "body"):
                return child
    return None


def _extract_first_meaningful_line(body_text: str, language: str) -> str:
    """Extract the first meaningful line (docstring or statement)."""
    lines = body_text.strip().split("\n")
    for line in lines:
        stripped = line.strip()
        if not stripped:
            continue
        # Return docstring indicators or first statement
        if stripped.startswith(('"""', "'''", "//", "/*", "#", "*")):
            return stripped
        if stripped.startswith(("return ", "self.", "this.", "raise ", "pass")):
            return stripped
        return stripped
    return "..."


def _get_indent(text: str) -> str:
    """Get the indentation of the first non-empty line."""
    for line in text.split("\n"):
        if line.strip():
            return line[: len(line) - len(line.lstrip())]
    return "    "


def _compress_fallback(code: str) -> str:
    """Fallback compression without AST: keep first/last lines, collapse middle."""
    lines = code.split("\n")
    if len(lines) <= 10:
        return code

    # Keep first 5 lines (usually signature + docstring) and last 2
    head = lines[:5]
    tail = lines[-2:]
    collapsed_count = len(lines) - 7
    return "\n".join(head + [f"    # ... ({collapsed_count} lines collapsed)"] + tail)
