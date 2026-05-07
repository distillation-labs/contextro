"""Smart context-aware chunking for improved retrieval quality.

Goes beyond simple one-chunk-per-symbol by creating overlapping context
windows that capture cross-function relationships. This addresses the
key insight from Augment Code: "callsites are not necessarily similar
to function definitions" — our chunks need to capture the relationship
context, not just the symbol in isolation.

Strategies:
1. Symbol chunks (existing) — one chunk per function/class
2. Relationship chunks — capture caller→callee pairs together
3. File-level context chunks — module docstring + imports + top-level structure
4. Signature clusters — group related functions by shared parameters/return types
"""

import hashlib
import logging
from typing import Dict, List, Set

from contextia_mcp.config import get_settings
from contextia_mcp.core.models import Symbol
from contextia_mcp.indexing.chunk_context import (
    ChunkContextSettings,
    module_hint_from_path,
    normalize_chunk_path,
)
from contextia_mcp.indexing.chunker import CodeChunk

logger = logging.getLogger(__name__)


def create_relationship_chunks(
    symbols: List[Symbol],
    max_chars: int = 3000,
) -> List[CodeChunk]:
    """Create chunks that capture caller→callee relationships.

    For each function that calls other functions, create a chunk containing:
    - The caller's signature + first few lines
    - The callee signatures it references
    - The import context

    This helps semantic search find code by what it DOES (calls) rather
    than just what it IS (its own signature).
    """
    chunks = []
    context_settings = ChunkContextSettings.from_settings(get_settings())
    # Build a lookup of symbol names to their signatures
    sig_lookup: Dict[str, str] = {}
    for sym in symbols:
        sig_lookup[sym.name] = sym.signature or f"{sym.type.value} {sym.name}"

    for sym in symbols:
        if not sym.calls:
            continue

        # Build relationship text
        file_hint = normalize_chunk_path(sym.filepath, context_settings.path_depth)
        module_hint = module_hint_from_path(sym.filepath, context_settings.path_depth)
        parts = [
            f"# file: {file_hint}",
            f"# relationship: {sym.qualified_name} calls",
        ]
        if module_hint:
            parts.append(f"# module: {module_hint}")

        # Add caller signature
        if sym.signature:
            parts.append(f"\n{sym.signature}")

        # Add callee signatures
        for callee_name in sym.calls[:10]:  # Limit to 10 callees
            callee_sig = sig_lookup.get(callee_name, callee_name)
            parts.append(f"  → {callee_sig}")

        # Add imports for context
        if sym.imports:
            parts.append(f"\nImports: {', '.join(sym.imports[:10])}")

        text = "\n".join(parts)
        if len(text) > max_chars:
            text = text[:max_chars]

        chunk_id = hashlib.sha256(
            f"rel:{sym.filepath}:{sym.name}:{sym.line_start}".encode()
        ).hexdigest()[:16]

        chunks.append(CodeChunk(
            id=chunk_id,
            text=text,
            filepath=sym.filepath,
            symbol_name=f"[rel] {sym.qualified_name}",
            symbol_type="relationship",
            language=sym.language,
            line_start=sym.line_start,
            line_end=sym.line_end,
            signature=sym.signature,
            parent=sym.parent or "",
            docstring="",
        ))

    return chunks


def create_file_context_chunks(
    symbols: List[Symbol],
    max_chars: int = 3000,
) -> List[CodeChunk]:
    """Create file-level context chunks that capture module structure.

    For each file with multiple symbols, create a chunk containing:
    - Module-level docstring (if any)
    - All imports
    - All function/class signatures (table of contents)

    This helps queries like "what does this module do?" or
    "where is the auth logic?" find the right file.
    """
    # Group symbols by file
    by_file: Dict[str, List[Symbol]] = {}
    for sym in symbols:
        by_file.setdefault(sym.filepath, []).append(sym)

    chunks = []
    context_settings = ChunkContextSettings.from_settings(get_settings())
    for filepath, file_symbols in by_file.items():
        if len(file_symbols) < 2:
            continue  # Skip single-symbol files

        file_hint = normalize_chunk_path(filepath, context_settings.path_depth)
        module_hint = module_hint_from_path(filepath, context_settings.path_depth)
        parts = [f"# File overview: {file_hint}"]
        if module_hint:
            parts.append(f"# module: {module_hint}")

        # Collect all imports
        all_imports: Set[str] = set()
        for sym in file_symbols:
            all_imports.update(sym.imports)

        if all_imports:
            parts.append(f"\nImports: {', '.join(sorted(all_imports)[:20])}")

        # Add all signatures as a table of contents
        parts.append("\n## Symbols:")
        for sym in sorted(file_symbols, key=lambda s: s.line_start):
            sig = sym.signature or f"{sym.type.value} {sym.name}"
            doc_preview = ""
            if sym.docstring:
                first_line = sym.docstring.split("\n")[0][:80]
                doc_preview = f" — {first_line}"
            parts.append(f"  L{sym.line_start}: {sig}{doc_preview}")

        # Add module docstring if the first symbol has one and is at line 1
        first_sym = min(file_symbols, key=lambda s: s.line_start)
        if first_sym.docstring and first_sym.line_start <= 3:
            parts.insert(1, f"\n{first_sym.docstring[:500]}")

        text = "\n".join(parts)
        if len(text) > max_chars:
            text = text[:max_chars]

        chunk_id = hashlib.sha256(
            f"file:{filepath}:overview".encode()
        ).hexdigest()[:16]

        lang = file_symbols[0].language if file_symbols else "unknown"
        chunks.append(CodeChunk(
            id=chunk_id,
            text=text,
            filepath=filepath,
            symbol_name=f"[file] {filepath.split('/')[-1]}",
            symbol_type="file_overview",
            language=lang,
            line_start=1,
            line_end=max(s.line_end for s in file_symbols),
            signature="",
            parent="",
            docstring="",
        ))

    return chunks


def create_smart_chunks(
    symbols: List[Symbol],
    include_relationships: bool = True,
    include_file_context: bool = True,
) -> List[CodeChunk]:
    """Create enhanced chunks with relationship and file context.

    This supplements (not replaces) the standard symbol chunks from chunker.py.
    The additional chunks improve retrieval for:
    - "What calls X?" queries (relationship chunks)
    - "What does this module do?" queries (file context chunks)
    - Cross-function dependency queries

    Args:
        symbols: List of parsed symbols.
        include_relationships: Create caller→callee relationship chunks.
        include_file_context: Create file-level overview chunks.

    Returns:
        List of additional CodeChunks to embed alongside standard chunks.
    """
    settings = get_settings()
    max_chars = settings.chunk_max_chars

    extra_chunks: List[CodeChunk] = []

    if include_relationships:
        rel_chunks = create_relationship_chunks(symbols, max_chars=max_chars)
        extra_chunks.extend(rel_chunks)
        logger.debug("Created %d relationship chunks", len(rel_chunks))

    if include_file_context:
        file_chunks = create_file_context_chunks(symbols, max_chars=max_chars)
        extra_chunks.extend(file_chunks)
        logger.debug("Created %d file context chunks", len(file_chunks))

    return extra_chunks
