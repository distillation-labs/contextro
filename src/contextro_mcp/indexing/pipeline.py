"""Indexing pipeline: discover → parse → chunk → embed → store.

Orchestrates tree-sitter (symbols for vectors) and ast-grep (graph structure)
parsing, embedding via ONNX, and storage in LanceDB + rustworkx.
"""

import json
import logging
import os
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Set

from contextro_mcp.config import Settings, get_settings
from contextro_mcp.core.graph_models import UniversalGraph
from contextro_mcp.engines.bm25_engine import LanceDBBM25Engine
from contextro_mcp.engines.graph_engine import RustworkxCodeGraph
from contextro_mcp.engines.vector_engine import LanceDBVectorEngine
from contextro_mcp.indexing.chunker import create_chunks
from contextro_mcp.indexing.embedding_service import get_embedding_service
from contextro_mcp.indexing.file_discovery import SKIP_DIRS, discover_files
from contextro_mcp.indexing.parallel_indexer import parallel_parse_files
from contextro_mcp.indexing.smart_chunker import create_smart_chunks
from contextro_mcp.parsing.astgrep_parser import AstGrepParser

logger = logging.getLogger(__name__)

__all__ = ["SKIP_DIRS", "discover_files", "IndexResult", "IndexingPipeline"]

METADATA_VERSION = 2
FINGERPRINTS_KEY = "fingerprints"
LEGACY_FILE_STATE_KEY = "mtimes"
CONTENT_HASH_DETECTION = "content_hash"


@dataclass
class IndexResult:
    """Result of an indexing operation."""

    total_files: int = 0
    total_symbols: int = 0
    total_chunks: int = 0
    parse_errors: int = 0
    graph_nodes: int = 0
    graph_relationships: int = 0
    time_seconds: float = 0.0
    files_added: int = 0
    files_modified: int = 0
    files_deleted: int = 0

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


def _transfer_graph(universal_graph: UniversalGraph, code_graph: RustworkxCodeGraph) -> None:
    """Transfer nodes and relationships from UniversalGraph to RustworkxCodeGraph."""
    for node in universal_graph.nodes.values():
        code_graph.add_node(node)
    for rel in universal_graph.relationships.values():
        code_graph.add_relationship(rel)


class IndexingPipeline:
    """Orchestrates the full indexing flow.

    Steps:
    1. Discover files (pathspec + extension filter)
    2. Parse symbols (tree-sitter, parallel)
    3. Parse graph structure (ast-grep, sequential)
    4. Chunk symbols into CodeChunks
    5. Embed chunks in batches
    6. Store in LanceDB + rustworkx graph
    7. Save metadata for incremental reindex
    8. Unload embedding model to free RAM
    """

    def __init__(self, settings: Optional[Settings] = None):
        self._settings = settings or get_settings()
        self._embedding_service = get_embedding_service(
            self._settings.embedding_model,
            batch_size=self._settings.embedding_batch_size,
            device=self._settings.embedding_device,
        )
        from contextro_mcp.indexing.embedding_service import EMBEDDING_MODELS

        model_config = EMBEDDING_MODELS.get(self._settings.embedding_model, {})
        self._vector_engine = LanceDBVectorEngine(
            db_path=str(self._settings.lancedb_path),
            embedding_service=self._embedding_service,
            vector_dims=model_config.get("dimensions", 768),
        )
        self._bm25_engine = LanceDBBM25Engine(
            db_path=str(self._settings.lancedb_path),
        )
        self._graph_engine = RustworkxCodeGraph()
        self._astgrep = AstGrepParser()
        self._metadata_path = self._settings.storage_path / "index_metadata.json"

    @property
    def vector_engine(self) -> LanceDBVectorEngine:
        return self._vector_engine

    @property
    def settings(self) -> Settings:
        return self._settings

    @property
    def bm25_engine(self) -> LanceDBBM25Engine:
        return self._bm25_engine

    @property
    def graph_engine(self) -> RustworkxCodeGraph:
        return self._graph_engine

    def _parse_astgrep_batch(self, batch_files: List[Path]) -> None:
        """Parse a file batch with ast-grep and merge it into the shared graph."""
        if self._settings.skip_astgrep:
            return

        parseable_files = [f for f in batch_files if self._astgrep.can_parse(str(f))]
        if not parseable_files:
            return

        from concurrent.futures import ThreadPoolExecutor, as_completed

        universal_graph = UniversalGraph()
        max_workers = min(self._settings.max_workers or os.cpu_count() or 8, len(parseable_files))

        def _parse_one(filepath: Path) -> UniversalGraph:
            local_graph = UniversalGraph()
            try:
                self._astgrep.parse_file(str(filepath), local_graph)
            except Exception as exc:
                logger.warning("ast-grep failed for %s: %s", filepath, exc)
            return local_graph

        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            futures = [executor.submit(_parse_one, filepath) for filepath in parseable_files]
            for future in as_completed(futures):
                local_graph = future.result()
                for node in local_graph.nodes.values():
                    universal_graph.nodes[node.id] = node
                for rel in local_graph.relationships.values():
                    universal_graph.relationships[rel.id] = rel

        _transfer_graph(universal_graph, self._graph_engine)

    def _persist_graph(self) -> None:
        """Persist the current graph for warm-start recovery."""
        from contextro_mcp.persistence.store import GraphPersistence

        persistence = GraphPersistence(str(self._settings.graph_path))
        try:
            persistence.save(self._graph_engine)
        except Exception as exc:
            logger.warning("Failed to persist graph: %s", exc)

    def _ensure_symbols_in_graph(self, symbols: List) -> None:
        """Ensure all tree-sitter symbols have corresponding graph nodes.

        ast-grep misses functions with decorators, type annotations, or complex
        signatures. tree-sitter finds them all. This method adds any missing
        symbols to the graph so call edges can be resolved.
        """
        from contextro_mcp.core.graph_models import (
            NodeType,
            UniversalLocation,
            UniversalNode,
        )

        # Get names already in graph
        existing_names: set = {n.name for n in self._graph_engine.nodes.values()}

        added = 0
        for sym in symbols:
            if sym.name in existing_names:
                continue
            # Skip if it's a very common name (likely a method like __init__)
            if sym.name.startswith("__") and sym.name.endswith("__"):
                continue

            # Determine node type
            sym_type = sym.type.value.lower() if sym.type else "function"
            if "class" in sym_type:
                node_type = NodeType.CLASS
            else:
                node_type = NodeType.FUNCTION

            node_id = f"ts_func:{added + len(self._graph_engine.nodes)}"
            node = UniversalNode(
                id=node_id,
                name=sym.name,
                node_type=node_type,
                location=UniversalLocation(
                    file_path=sym.filepath,
                    start_line=sym.line_start,
                    end_line=sym.line_end,
                    language=sym.language,
                ),
                language=sym.language,
                line_count=sym.line_end - sym.line_start + 1 if sym.line_end else 1,
                docstring=sym.docstring[:200] if sym.docstring else None,
            )
            self._graph_engine.add_node(node)
            existing_names.add(sym.name)
            added += 1

    def _build_call_edges_post_pass(self, symbol_calls: List) -> None:
        """Build CALLS edges after all symbols are in the graph.

        This post-pass approach ensures cross-file call resolution works:
        when function A in file 1 calls function B in file 2, both must
        be in the graph before we can create the edge.

        Args:
            symbol_calls: List of (caller_name, calls_tuple) pairs.
        """
        from contextro_mcp.core.graph_models import (
            RelationshipType,
            UniversalRelationship,
        )

        # Build complete name -> node_id lookup from the fully-populated graph
        known_symbols: Dict[str, str] = {}
        for node in self._graph_engine.nodes.values():
            known_symbols[node.name] = node.id

        rel_count = 0
        seen_edges: set = set()

        for caller_name, calls in symbol_calls:
            caller_id = known_symbols.get(caller_name)
            if not caller_id:
                continue

            for called_name in calls:
                target_id = known_symbols.get(called_name)
                if not target_id:
                    continue
                if caller_id == target_id:
                    continue

                edge_key = f"{caller_id}:{target_id}"
                if edge_key in seen_edges:
                    continue
                seen_edges.add(edge_key)

                try:
                    self._graph_engine.add_relationship(
                        UniversalRelationship(
                            id=f"call:{rel_count}",
                            source_id=caller_id,
                            target_id=target_id,
                            relationship_type=RelationshipType.CALLS,
                        )
                    )
                    rel_count += 1
                except Exception:
                    continue

        logger.info("Built %d CALLS edges (post-pass)", rel_count)

    def _build_call_edges_from_symbols(self, symbols: List) -> None:
        """Build CALLS edges in the graph from tree-sitter symbol.calls data.

        This is more reliable than ast-grep patterns because tree-sitter
        correctly handles decorated functions, type annotations, async defs, etc.
        """
        from contextro_mcp.core.graph_models import (
            RelationshipType,
            UniversalRelationship,
        )

        # Build a name -> node_id lookup for all known graph nodes
        known_symbols: Dict[str, str] = {}
        for node in self._graph_engine.nodes.values():
            known_symbols[node.name] = node.id

        # Also index the symbols being processed (they may not be in graph yet)
        symbol_name_to_file: Dict[str, str] = {}
        for sym in symbols:
            symbol_name_to_file[sym.name] = sym.filepath

        rel_count = 0
        for sym in symbols:
            if not sym.calls:
                continue

            # Find the caller's node ID in the graph (by name match)
            caller_id = known_symbols.get(sym.name)
            if not caller_id:
                continue  # Skip if caller isn't in the graph

            for called_name in sym.calls:
                # Resolve the target by name
                target_id = known_symbols.get(called_name)
                if not target_id:
                    continue  # Skip unresolved calls (external/builtin)

                # Skip self-calls
                if caller_id == target_id:
                    continue

                try:
                    self._graph_engine.add_relationship(
                        UniversalRelationship(
                            id=f"call:{rel_count}",
                            source_id=caller_id,
                            target_id=target_id,
                            relationship_type=RelationshipType.CALLS,
                        )
                    )
                    rel_count += 1
                except Exception:
                    continue

    def _embed_and_store_symbols(self, symbols: List, batch_size: int) -> int:
        """Chunk, embed, and store symbols. Returns chunk count.

        Creates both standard symbol chunks and smart context chunks
        (relationship + file overview) for improved retrieval quality.
        Accumulates all chunks and writes to LanceDB in one batch for speed.
        """
        total_chunks = 0
        all_chunk_dicts: List = []

        # Standard symbol chunks
        for i in range(0, len(symbols), batch_size):
            batch_chunks = create_chunks(symbols[i : i + batch_size])
            if not batch_chunks:
                continue
            total_chunks += len(batch_chunks)
            vectors = self._embedding_service.embed_batch([c.text for c in batch_chunks])
            for chunk, vector in zip(batch_chunks, vectors):
                chunk.vector = vector
            all_chunk_dicts.extend(c.to_dict() for c in batch_chunks)

        # Smart context chunks (relationship + file overview)
        smart_chunks = create_smart_chunks(
            symbols,
            include_relationships=self._settings.smart_chunk_relationships_enabled,
            include_file_context=self._settings.smart_chunk_file_context_enabled,
        )
        if smart_chunks:
            for i in range(0, len(smart_chunks), batch_size):
                batch = smart_chunks[i : i + batch_size]
                vectors = self._embedding_service.embed_batch([c.text for c in batch])
                for chunk, vector in zip(batch, vectors):
                    chunk.vector = vector
                all_chunk_dicts.extend(c.to_dict() for c in batch)
            total_chunks += len(smart_chunks)

        # Write all chunks to LanceDB in one batch (much faster than many small writes)
        if all_chunk_dicts:
            self._vector_engine.add(all_chunk_dicts)

        return total_chunks

    def index(
        self,
        codebase_path: Path,
        progress_callback: Optional[Callable[[str, Dict[str, Any]], None]] = None,
    ) -> IndexResult:
        """Full index of a codebase. Streams files in batches to limit peak RAM."""
        start = time.time()
        codebase_path = Path(codebase_path).resolve()

        # Ensure storage dir exists
        self._settings.storage_path.mkdir(parents=True, exist_ok=True)

        # Step 1: Discover files
        files = discover_files(codebase_path, self._settings)
        logger.info("Discovered %d files in %s", len(files), codebase_path)

        if not files:
            return IndexResult(time_seconds=time.time() - start)

        # Clear engines before indexing
        self._graph_engine.clear()
        self._vector_engine.clear()

        total_symbols = 0
        total_chunks = 0
        total_parse_errors = 0
        file_batch_size = self._settings.index_file_batch_size
        embed_batch_size = self._settings.embedding_batch_size
        all_symbols_for_graph: List = []  # Collect all symbols for post-pass call edge building

        try:
            # Stream files in batches: parse → graph → embed → store per batch
            for batch_start in range(0, len(files), file_batch_size):
                batch_files = files[batch_start : batch_start + file_batch_size]

                # Parse symbols with tree-sitter (parallel)
                symbols, parse_errors = parallel_parse_files(
                    batch_files, self._settings.max_workers, progress_callback
                )
                total_parse_errors += parse_errors
                total_symbols += len(symbols)

                self._parse_astgrep_batch(batch_files)

                # Ensure all tree-sitter symbols are in the graph (handles
                # decorated/typed functions that ast-grep misses)
                self._ensure_symbols_in_graph(symbols)

                # Collect symbols for post-pass call edge building
                # (must happen after ALL batches so cross-file calls resolve)
                all_symbols_for_graph.extend((s.name, s.calls) for s in symbols if s.calls)

                # Chunk, embed, store
                total_chunks += self._embed_and_store_symbols(symbols, embed_batch_size)
                del symbols

                logger.info(
                    "Batch %d-%d: %d files processed",
                    batch_start,
                    batch_start + len(batch_files),
                    len(batch_files),
                )

            # Post-pass: build CALLS edges now that ALL symbols are in the graph
            # This ensures cross-file call resolution works correctly
            self._build_call_edges_post_pass(all_symbols_for_graph)
            del all_symbols_for_graph

            graph_stats = self._graph_engine.get_statistics()

            if total_chunks > 0:
                # Create FTS index for BM25
                self._bm25_engine.clear()
                self._bm25_engine.ensure_fts_index()

            # Save metadata
            self._save_metadata(codebase_path, files)

            self._persist_graph()
        finally:
            # Always unload model to free RAM
            self._embedding_service.unload()

        elapsed = time.time() - start
        logger.info(
            "Indexed %d files, %d symbols, %d chunks in %.1fs",
            len(files),
            total_symbols,
            total_chunks,
            elapsed,
        )

        return IndexResult(
            total_files=len(files),
            total_symbols=total_symbols,
            total_chunks=total_chunks,
            parse_errors=total_parse_errors,
            graph_nodes=graph_stats["total_nodes"],
            graph_relationships=graph_stats["total_relationships"],
            time_seconds=elapsed,
        )

    def _validate_index(self) -> bool:
        """Validate that stored index artifacts are intact.

        Returns True if valid, False if corrupt or missing.
        """
        # Check metadata file
        metadata = self._load_metadata_payload()
        if metadata is None:
            return False
        if self._extract_file_state(metadata) is None:
            return False

        # Check vector engine table schema
        if not self._vector_engine.validate():
            logger.warning("Corrupt vector index detected.")
            return False

        return True

    def incremental_index(
        self,
        codebase_path: Path,
        progress_callback: Optional[Callable[[str, Dict[str, Any]], None]] = None,
    ) -> IndexResult:
        """Incrementally re-index only changed/new/deleted files."""
        start = time.time()
        codebase_path = Path(codebase_path).resolve()

        # Warm-start: always try to load graph from SQLite persistence first
        # This ensures we have the full graph context before any deletions
        from contextro_mcp.persistence.store import GraphPersistence

        persistence = GraphPersistence(str(self._settings.graph_path))
        if persistence.exists():
            loaded_graph = persistence.load()
            if loaded_graph and loaded_graph.get_statistics().get("total_nodes", 0) > 0:
                self._graph_engine = loaded_graph
                logger.info(
                    "Warm-started graph from persistence (%d nodes)",
                    loaded_graph.get_statistics().get("total_nodes", 0),
                )

        # Also ensure BM25 FTS index is ready
        if not self._bm25_engine._fts_index_created:
            try:
                self._bm25_engine.ensure_fts_index()
            except Exception:
                pass

        # Validate existing index integrity
        if not self._validate_index():
            logger.warning("Corrupt index detected, performing full rebuild.")
            try:
                self._metadata_path.unlink(missing_ok=True)
            except OSError:
                pass
            return self.index(codebase_path, progress_callback)

        # Load stored metadata
        stored_metadata = self._load_metadata_payload()
        stored_file_state = (
            self._extract_file_state(stored_metadata) if stored_metadata is not None else None
        )
        if stored_file_state is None:
            return self.index(codebase_path, progress_callback)

        # Discover current files and compute file-state fingerprints.
        files = discover_files(codebase_path, self._settings)

        from contextro_mcp.accelerator import diff_mtimes_fast

        current_fingerprints = self._compute_file_fingerprints(files)
        using_legacy_mtimes = stored_metadata is not None and self._metadata_uses_legacy_mtimes(
            stored_metadata
        )

        if using_legacy_mtimes:
            current_file_state = self._compute_file_mtimes(files)
            logger.info(
                "Legacy mtime metadata detected; diffing once with mtimes before "
                "upgrading to content hashes"
            )
        else:
            current_file_state = current_fingerprints
            logger.info("Diffing %d files with content hashes", len(files))

        added_list, modified_list, deleted_list = diff_mtimes_fast(
            current_file_state,
            stored_file_state,
        )
        new_files = set(added_list)
        modified_files = set(modified_list)
        deleted_files = set(deleted_list)
        logger.info(
            "Incremental diff: %d new, %d modified, %d deleted",
            len(new_files),
            len(modified_files),
            len(deleted_files),
        )

        changed_files = new_files | modified_files

        if not changed_files and not deleted_files:
            if using_legacy_mtimes:
                self._write_metadata(
                    codebase_path=codebase_path,
                    fingerprints=current_fingerprints,
                )
            # No changes — return stats from existing index
            graph_stats = self._graph_engine.get_statistics()
            self._embedding_service.unload()
            return IndexResult(
                total_files=len(files),
                total_symbols=graph_stats.get("total_nodes", 0),
                total_chunks=self._vector_engine.count(),
                graph_nodes=graph_stats.get("total_nodes", 0),
                graph_relationships=graph_stats.get("total_relationships", 0),
                time_seconds=time.time() - start,
            )

        # Remove deleted/modified from engines
        for fp in deleted_files | modified_files:
            self._vector_engine.delete_by_filepath(fp)
            self._graph_engine.remove_file_nodes(fp)

        # Parse and index changed files in streaming batches
        changed_paths = [Path(f) for f in changed_files]
        total_symbols = 0
        total_chunks = 0
        parse_errors = 0
        file_batch_size = self._settings.index_file_batch_size
        embed_batch_size = self._settings.embedding_batch_size
        all_symbols_for_graph: List = []  # For post-pass call edge building

        try:
            for batch_start in range(0, len(changed_paths), file_batch_size):
                batch_files = changed_paths[batch_start : batch_start + file_batch_size]

                symbols, batch_errors = parallel_parse_files(
                    batch_files, self._settings.max_workers, progress_callback
                )
                parse_errors += batch_errors
                total_symbols += len(symbols)

                self._parse_astgrep_batch(batch_files)

                # Ensure tree-sitter symbols are in the graph
                self._ensure_symbols_in_graph(symbols)

                # Collect for post-pass call edge building
                all_symbols_for_graph.extend((s.name, s.calls) for s in symbols if s.calls)

                total_chunks += self._embed_and_store_symbols(symbols, embed_batch_size)
                del symbols

            # Post-pass: build CALLS edges with complete graph
            self._build_call_edges_post_pass(all_symbols_for_graph)
            del all_symbols_for_graph

            # Rebuild FTS index for BM25
            self._bm25_engine.clear()
            self._bm25_engine.ensure_fts_index()

            # Save updated metadata
            self._save_metadata(codebase_path, files)

            self._persist_graph()
        finally:
            self._embedding_service.unload()

        graph_stats = self._graph_engine.get_statistics()
        elapsed = time.time() - start

        return IndexResult(
            total_files=len(files),
            total_symbols=total_symbols,
            total_chunks=total_chunks,
            parse_errors=parse_errors,
            graph_nodes=graph_stats["total_nodes"],
            graph_relationships=graph_stats["total_relationships"],
            time_seconds=elapsed,
            files_added=len(new_files),
            files_modified=len(modified_files),
            files_deleted=len(deleted_files),
        )

    def multi_index(
        self,
        codebase_paths: List[Path],
        progress_callback: Optional[Callable[[str, Dict[str, Any]], None]] = None,
    ) -> IndexResult:
        """Index multiple directories folder-by-folder into shared engines.

        Processes each folder sequentially: discover → parse → embed → store,
        then builds the FTS index and saves metadata once at the end.
        This keeps peak RAM low since each folder's symbols are freed before
        the next folder starts.

        Args:
            codebase_paths: List of absolute paths to index.
            progress_callback: Optional callback for progress events.

        Returns:
            Aggregated IndexResult across all paths.
        """
        if not codebase_paths:
            return IndexResult()

        if len(codebase_paths) == 1:
            return self.index(codebase_paths[0], progress_callback)

        start = time.time()
        resolved_paths = [Path(p).resolve() for p in codebase_paths]

        # Ensure storage dir exists
        self._settings.storage_path.mkdir(parents=True, exist_ok=True)

        # Clear engines once before processing all folders
        self._graph_engine.clear()
        self._vector_engine.clear()

        total_files = 0
        total_symbols = 0
        total_chunks = 0
        total_parse_errors = 0
        all_indexed_files: List[Path] = []
        seen_files: Set[str] = set()
        all_symbols_for_graph: List = []

        file_batch_size = self._settings.index_file_batch_size
        embed_batch_size = self._settings.embedding_batch_size

        try:
            for folder_idx, root in enumerate(resolved_paths):
                logger.info(
                    "Indexing folder %d/%d: %s",
                    folder_idx + 1,
                    len(resolved_paths),
                    root,
                )

                # Step 1: Discover files for this folder
                files = discover_files(root, self._settings)
                # Deduplicate across folders (e.g. overlapping paths)
                files = [f for f in files if str(f) not in seen_files]
                for f in files:
                    seen_files.add(str(f))
                all_indexed_files.extend(files)

                if not files:
                    logger.info("No files found in %s, skipping", root)
                    continue

                total_files += len(files)

                # Stream files in batches within each folder
                for batch_start in range(0, len(files), file_batch_size):
                    batch_files = files[batch_start : batch_start + file_batch_size]

                    # Parse symbols with tree-sitter (parallel)
                    symbols, parse_errors = parallel_parse_files(
                        batch_files, self._settings.max_workers, progress_callback
                    )
                    total_parse_errors += parse_errors
                    total_symbols += len(symbols)

                    self._parse_astgrep_batch(batch_files)
                    self._ensure_symbols_in_graph(symbols)
                    all_symbols_for_graph.extend(
                        (symbol.name, symbol.calls) for symbol in symbols if symbol.calls
                    )

                    # Chunk, embed, store
                    total_chunks += self._embed_and_store_symbols(symbols, embed_batch_size)
                    del symbols

                logger.info(
                    "Folder %s: %d files processed",
                    root.name,
                    len(files),
                )

            self._build_call_edges_post_pass(all_symbols_for_graph)

            # Build FTS index once after all folders
            if total_chunks > 0:
                self._bm25_engine.clear()
                self._bm25_engine.ensure_fts_index()

            # Save metadata for all roots
            self._save_multi_metadata(resolved_paths, all_indexed_files)
            self._persist_graph()
        finally:
            # Always unload model to free RAM
            self._embedding_service.unload()

        graph_stats = self._graph_engine.get_statistics()
        elapsed = time.time() - start
        logger.info(
            "Multi-indexed %d files, %d symbols, %d chunks from %d folders in %.1fs",
            total_files,
            total_symbols,
            total_chunks,
            len(resolved_paths),
            elapsed,
        )

        return IndexResult(
            total_files=total_files,
            total_symbols=total_symbols,
            total_chunks=total_chunks,
            parse_errors=total_parse_errors,
            graph_nodes=graph_stats["total_nodes"],
            graph_relationships=graph_stats["total_relationships"],
            time_seconds=elapsed,
        )

    def _save_multi_metadata(self, codebase_paths: List[Path], files: List[Path]) -> None:
        """Save file fingerprints for multi-root indexing metadata."""
        self._write_metadata(
            codebase_path=codebase_paths[0],
            codebase_paths=codebase_paths,
            fingerprints=self._compute_file_fingerprints(files),
        )

    def _save_metadata(self, codebase_path: Path, files: List[Path]) -> None:
        """Save file fingerprints for content-hash incremental reindex."""
        self._write_metadata(
            codebase_path=codebase_path,
            fingerprints=self._compute_file_fingerprints(files),
        )

    def _load_metadata_payload(self) -> Optional[Dict[str, Any]]:
        """Load raw metadata JSON, or None if missing/corrupt."""
        if not self._metadata_path.exists():
            return None
        try:
            data = json.loads(self._metadata_path.read_text())
        except (json.JSONDecodeError, OSError):
            logger.warning("Corrupt index metadata file.")
            return None
        return data if isinstance(data, dict) else None

    def _extract_file_state(self, metadata: Dict[str, Any]) -> Optional[Dict[str, str | float]]:
        """Extract persisted file-state data from new or legacy metadata."""
        for key in (FINGERPRINTS_KEY, LEGACY_FILE_STATE_KEY):
            raw_state = metadata.get(key)
            if raw_state is None:
                continue
            if not isinstance(raw_state, dict):
                return None

            normalized: Dict[str, str | float] = {}
            for path, value in raw_state.items():
                if not isinstance(path, str):
                    return None
                if isinstance(value, bool) or not isinstance(value, (str, int, float)):
                    return None
                normalized[path] = value
            return normalized

        return None

    def _metadata_uses_legacy_mtimes(self, metadata: Dict[str, Any]) -> bool:
        """Return True when metadata still uses the old mtime payload."""
        return LEGACY_FILE_STATE_KEY in metadata and FINGERPRINTS_KEY not in metadata

    def _compute_file_fingerprints(self, files: List[Path]) -> Dict[str, str]:
        """Hash file contents for reliable change detection."""
        from contextro_mcp.accelerator import hash_files_fast

        return hash_files_fast([str(f) for f in files])

    def _compute_file_mtimes(self, files: List[Path]) -> Dict[str, float]:
        """Read file mtimes for a one-time legacy metadata upgrade path."""
        from contextro_mcp.accelerator import scan_mtimes_fast

        return scan_mtimes_fast([str(f) for f in files])

    def _write_metadata(
        self,
        *,
        codebase_path: Path,
        fingerprints: Dict[str, str],
        codebase_paths: Optional[List[Path]] = None,
    ) -> None:
        """Write normalized metadata for content-hash incremental indexing."""
        metadata: Dict[str, Any] = {
            "metadata_version": METADATA_VERSION,
            "change_detection": CONTENT_HASH_DETECTION,
            "codebase_path": str(codebase_path),
            FINGERPRINTS_KEY: fingerprints,
        }
        if codebase_paths is not None:
            metadata["codebase_paths"] = [str(p) for p in codebase_paths]
        self._metadata_path.parent.mkdir(parents=True, exist_ok=True)
        self._metadata_path.write_text(json.dumps(metadata))

    def _load_metadata(self) -> Optional[Dict[str, str | float]]:
        """Load stored file-state metadata, or None if none exists."""
        metadata = self._load_metadata_payload()
        if metadata is None:
            return None
        return self._extract_file_state(metadata)
