"""Contextro FastMCP server with index, search, status, and graph/analysis tools."""

import argparse
import json as _json
import logging
import os
import re
import resource
import signal
import sys
import threading
from pathlib import Path
from typing import Annotated, Any, List, Optional

logger = logging.getLogger(__name__)


class JsonFormatter(logging.Formatter):
    """JSON structured log formatter for production use."""

    def format(self, record: logging.LogRecord) -> str:
        log_entry = {
            "timestamp": self.formatTime(record),
            "level": record.levelname,
            "logger": record.name,
            "message": record.getMessage(),
        }
        if record.exc_info and record.exc_info[0] is not None:
            log_entry["exception"] = self.formatException(record.exc_info)
        return _json.dumps(log_entry)


# Module-level pipeline reference (persists across tool calls)
_pipeline = None
_pipeline_lock = threading.Lock()

# Memory store initialization can happen from concurrent tool calls.
_memory_store_lock = threading.Lock()
_compaction_archive_lock = threading.Lock()

# Background indexing state
_index_job: dict = {}  # {"status": "indexing"|"done"|"error", "result": ..., "error": ...}
_index_job_lock = threading.Lock()

# Update check cache: (latest_version_str, checked_at_timestamp)
_update_cache: tuple[str, float] | None = None
_update_cache_lock = threading.Lock()
_UPDATE_CACHE_TTL = 3600.0  # re-check PyPI at most once per hour


def _check_for_update() -> str | None:
    """Return the latest PyPI version if newer than current, else None.

    Result is cached for 1 hour and fetched in a background thread so it
    never blocks the status() response.
    """
    import time

    from contextro_mcp import __version__

    global _update_cache

    with _update_cache_lock:
        now = time.monotonic()
        if _update_cache is not None:
            latest, checked_at = _update_cache
            if now - checked_at < _UPDATE_CACHE_TTL:
                # Return cached result
                try:
                    from packaging.version import Version
                    return latest if Version(latest) > Version(__version__) else None
                except Exception:
                    return None

    # Fetch from PyPI in a background thread; return None immediately if not ready
    def _fetch() -> None:
        global _update_cache
        try:
            import json as _json
            import time as _time
            import urllib.request

            with urllib.request.urlopen(
                "https://pypi.org/pypi/contextro/json", timeout=3
            ) as resp:
                data = _json.loads(resp.read())
            latest = data["info"]["version"]
            with _update_cache_lock:
                _update_cache = (latest, _time.monotonic())
        except Exception:
            pass  # Network unavailable — silently skip

    t = threading.Thread(target=_fetch, daemon=True, name="contextro-update-check")
    t.start()
    return None  # First call returns None; subsequent calls use the cache


def create_server():
    """Create and configure the FastMCP server."""
    from fastmcp import FastMCP
    from fastmcp.tools.tool import ToolResult
    from mcp.types import TextContent

    from contextro_mcp.core.graph_models import UniversalNode, UniversalRelationship

    mcp = FastMCP("Contextro")

    def _fmt_text(data: dict) -> str:
        """Render a tool result dict as a compact human-readable string.

        Produces a key: value block that agents can read without parsing JSON.
        Nested dicts are indented one level; lists are comma-joined or line-listed.
        """
        lines = []
        for k, v in data.items():
            if isinstance(v, dict):
                lines.append(f"{k}:")
                for dk, dv in v.items():
                    lines.append(f"  {dk}: {dv}")
            elif isinstance(v, list):
                if not v:
                    lines.append(f"{k}: (none)")
                elif all(isinstance(i, str) for i in v):
                    lines.append(f"{k}: {', '.join(v)}")
                else:
                    lines.append(f"{k}:")
                    for item in v:
                        if isinstance(item, dict):
                            # compact single-line for small dicts (search results etc.)
                            parts = "  " + "  ".join(
                                f"{ik}: {iv}"
                                for ik, iv in item.items()
                                if iv not in (None, "", [], {})
                            )
                            lines.append(parts)
                        else:
                            lines.append(f"  {item}")
            else:
                lines.append(f"{k}: {v}")
        return "\n".join(lines)

    def _fmt_search(data: dict) -> str:
        """Human-readable formatter for search results."""
        lines = [
            f"query: {data.get('query', '')}",
            f"confidence: {data.get('confidence', '?')}  total: {data.get('total', 0)}  tokens: {data.get('tokens', '?')}",
        ]
        if data.get("sandboxed"):
            lines.append(
                f"[sandboxed — {data.get('full_total')} results, sandbox_ref: {data.get('sandbox_ref')}]"
            )
            lines.append(data.get("hint", ""))
        for r in data.get("results", []):
            lines.append("")
            lines.append(
                f"  {r.get('name', r.get('symbol_name', '?'))}  ({r.get('file', r.get('filepath', '?'))}:{r.get('line', r.get('line_start', '?'))})"
            )
            lines.append(
                f"  type: {r.get('language', '?')} {r.get('symbol_type', '')}  score: {r.get('score', '?')}  match: {r.get('match', '?')}"
            )
            code = r.get("code", "")
            if code:
                lines.append("  ---")
                for cl in code.splitlines()[:6]:
                    lines.append(f"  {cl}")
        return "\n".join(lines)

    def _fmt_symbols(data: dict) -> str:
        """Human-readable formatter for find_symbol results."""
        lines = [f"total: {data.get('total', 0)}"]
        if data.get("note"):
            lines.append(f"note: {data['note']}")
        for s in data.get("symbols", []):
            lines.append("")
            lines.append(f"  {s.get('name')}  ({s.get('file')}:{s.get('line')})")
            lines.append(
                f"  type: {s.get('type')}  language: {s.get('language', 'python')}  lines: {s.get('line_count', '?')}"
            )
            if s.get("top_callers"):
                lines.append(
                    f"  callers ({s.get('callers_count', 0)}): {', '.join(s['top_callers'])}"
                )
            if s.get("top_callees"):
                lines.append(
                    f"  callees ({s.get('callees_count', 0)}): {', '.join(s['top_callees'])}"
                )
            if s.get("docstring"):
                lines.append(f"  doc: {s['docstring'][:120]}")
        return "\n".join(lines)

    def _tool_result(data: dict, fmt: str = "default") -> ToolResult:
        """Return a ToolResult with human-readable text + structured content."""
        if fmt == "search":
            text = _fmt_search(data)
        elif fmt == "symbols":
            text = _fmt_symbols(data)
        else:
            text = _fmt_text(data)
        return ToolResult(
            content=[TextContent(type="text", text=text)],
            structured_content=data,
        )

    def _apply_disclosure(
        data: dict,
        *,
        tool_name: str = "",
        preview_keys: list | None = None,
        max_list_items: int = 5,
    ) -> dict:
        """Apply progressive disclosure: sandbox large responses, return preview.

        Returns the original data if small, or a compact preview with sandbox_ref.
        """
        from contextro_mcp.engines.output_sandbox import OutputSandbox
        from contextro_mcp.execution.response_policy import ToolResponsePolicy
        from contextro_mcp.state import get_state

        state = get_state()
        if not hasattr(state, "_output_sandbox") or state._output_sandbox is None:
            state._output_sandbox = OutputSandbox(
                max_entries=_settings.search_sandbox_max_entries,
                ttl=_settings.search_sandbox_ttl_seconds,
            )

        policy = ToolResponsePolicy(
            output_sandbox=state._output_sandbox,
            threshold_tokens=_settings.search_sandbox_threshold_tokens,
        )
        return policy.apply(
            data,
            tool_name=tool_name,
            preview_keys=preview_keys,
            max_list_items=max_list_items,
        )

    # --- Middleware: permissions, rate limiting, audit ---

    from contextro_mcp.config import get_settings as _get_settings

    _settings = _get_settings()

    # Initialize audit logger
    from contextro_mcp.middleware.audit import AuditLogger

    _audit = AuditLogger(enabled=_settings.audit_enabled)

    # Initialize rate limiter (off by default for stdio)
    _rate_limiter = None
    if _settings.rate_limit_enabled:
        from contextro_mcp.security.rate_limiter import TokenBucketRateLimiter

        _rate_limiter = TokenBucketRateLimiter(
            default_rate=_settings.rate_limit_default_rate,
            default_burst=_settings.rate_limit_default_burst,
        )

    def _check_tool_permission(tool_name: str) -> Optional[dict]:
        """Check if tool is allowed under current permission policy.

        Returns None if allowed, or error dict if denied.
        """
        from contextro_mcp.security.permissions import (
            check_permission,
            get_tool_category,
            policy_from_level,
        )

        policy = policy_from_level(_settings.default_permission_level)
        if not check_permission(tool_name, policy):
            category = get_tool_category(tool_name)
            cat_name = category.value if category else "unknown"
            return {
                "error": (
                    f"Permission denied: tool '{tool_name}' "
                    f"requires '{cat_name}' access. "
                    f"Set CTX_PERMISSION_LEVEL=full to enable."
                )
            }
        return None

    def _check_rate_limit(tool_name: str) -> Optional[dict]:
        """Check rate limit for a tool. Returns error dict if limited."""
        if _rate_limiter is None:
            return None
        if not _rate_limiter.try_acquire(tool_name):
            retry = _rate_limiter.get_retry_after(tool_name)
            return {
                "error": "Rate limit exceeded.",
                "retry_after": round(retry, 2),
            }
        return None

    def _guard(tool_name: str) -> Optional[dict]:
        """Run all pre-execution checks: permissions, rate limiting.

        Returns None if all checks pass, or error dict on first failure.
        """
        err = _check_tool_permission(tool_name)
        if err:
            return err
        err = _check_rate_limit(tool_name)
        if err:
            return err
        return None

    def _get_tracker():
        """Return the session tracker, creating it if needed."""
        from contextro_mcp.memory.session_tracker import SessionTracker
        from contextro_mcp.state import get_state

        state = get_state()
        if not hasattr(state, "_session_tracker") or state._session_tracker is None:
            state._session_tracker = SessionTracker()
        return state._session_tracker

    def _warm_search_pipeline(state, settings) -> None:
        """Load search-critical components before the first user query."""
        if not getattr(settings, "search_prewarm_enabled", True):
            return

        try:
            if state.vector_engine:
                state.vector_engine.search("warmup", limit=1)
        except Exception as exc:
            logger.debug("Vector pre-warm failed (non-critical): %s", exc)

        if not getattr(settings, "search_prewarm_reranker", True):
            return

        try:
            from contextro_mcp.engines.reranker import FlashReranker

            if not hasattr(state, "_reranker") or state._reranker is None:
                max_passage_chars = int(getattr(settings, "search_rerank_max_passage_chars", 0))
                state._reranker = FlashReranker(
                    model_name=settings.reranker_model,
                    max_passage_chars=max_passage_chars if max_passage_chars > 0 else None,
                )
            if state._reranker.available:
                state._reranker._load_ranker()
        except Exception as exc:
            logger.debug("Reranker pre-warm failed (non-critical): %s", exc)

    # --- Input validation helpers ---

    def _strip_trailing_sep(path: str) -> str:
        """Normalize trailing separators while preserving the filesystem root."""
        if path == os.sep:
            return path
        return path.rstrip(os.sep)

    def _candidate_path_forms(path: str) -> list[str]:
        """Return normalized path variants for matching host and container paths."""
        raw = Path(path.strip()).expanduser()
        candidates: list[Path] = [raw]
        try:
            candidates.append(raw.resolve(strict=False))
        except (OSError, ValueError):
            pass

        forms: list[str] = []
        for candidate in candidates:
            normalized = _strip_trailing_sep(str(candidate))
            if normalized and normalized not in forms:
                forms.append(normalized)
        return forms

    def _configured_path_mappings() -> list[tuple[str, str]]:
        """Collect explicit and inferred host-to-container path mappings."""
        raw_entries = [
            entry.strip()
            for entry in os.environ.get("CTX_PATH_PREFIX_MAP", "").split(";")
            if entry.strip()
        ]

        host_path = os.environ.get("CTX_CODEBASE_HOST_PATH", "").strip()
        if host_path:
            mount_path = os.environ.get("CTX_CODEBASE_MOUNT_PATH", "/repos/platform").strip()
            raw_entries.append(f"{host_path}={mount_path}")

        mappings: list[tuple[str, str]] = []
        seen: set[tuple[str, str]] = set()

        for entry in raw_entries:
            if "=" not in entry:
                continue
            source, target = entry.split("=", 1)
            source_forms = _candidate_path_forms(source)
            try:
                normalized_target = _strip_trailing_sep(
                    str(Path(target.strip()).expanduser().resolve(strict=False))
                )
            except (OSError, ValueError):
                continue

            for normalized_source in source_forms:
                mapping = (normalized_source, normalized_target)
                if normalized_source and normalized_target and mapping not in seen:
                    mappings.append(mapping)
                    seen.add(mapping)

        return mappings

    def _validate_path(path: str) -> tuple[Optional[Path], Optional[dict]]:
        """Validate and resolve a codebase path.

        Returns (resolved_path, None) on success, or (None, error_dict) on failure.
        """
        if "\x00" in path:
            return None, {"error": "Path contains null bytes."}

        try:
            resolved = Path(path).resolve(strict=False)
        except (OSError, ValueError) as e:
            return None, {"error": f"Invalid path: {e}"}

        if not resolved.is_dir():
            remapped = _remap_path_prefix(path)
            if remapped is not None:
                return remapped, None
            if _configured_path_mappings():
                hint = (
                    " If Contextro is running in Docker, call index() with the host path and set "
                    "CTX_PATH_PREFIX_MAP=/host/path=/container/path or pass CTX_CODEBASE_HOST_PATH "
                    "and CTX_CODEBASE_MOUNT_PATH into the container."
                )
            else:
                hint = ""
            return None, {"error": f"Not a directory: {path}.{hint}"}

        return resolved, None

    def _remap_path_prefix(path: str) -> Optional[Path]:
        """Map a host path to a mounted path when running behind a container."""
        mappings = _configured_path_mappings()
        if not mappings:
            return None

        for normalized in _candidate_path_forms(path):
            for source, target in sorted(mappings, key=lambda pair: len(pair[0]), reverse=True):
                if normalized == source or normalized.startswith(source + os.sep):
                    suffix = normalized[len(source) :].lstrip(os.sep)
                    candidate = Path(target) / suffix if suffix else Path(target)
                    if candidate.is_dir():
                        return candidate.resolve(strict=False)

        return None

    def _validate_symbol_name(name: str) -> Optional[dict]:
        """Validate a symbol name. Returns error dict or None if valid."""
        if "\x00" in name:
            return {"error": "Symbol name contains null bytes."}
        if len(name) > 500:
            return {"error": "Symbol name too long (max 500 characters)."}
        if not name or not re.search(r"\w", name):
            return {"error": "Symbol name must contain at least one alphanumeric character."}
        return None

    def _validate_query(query: str) -> Optional[dict]:
        """Validate a search query. Returns error dict or None if valid."""
        if "\x00" in query:
            return {"error": "Query contains null bytes."}
        if len(query) > 10000:
            return {"error": "Query too long (max 10,000 characters)."}
        if not query.strip():
            return {"error": "Query must not be empty."}
        return None

    # --- Shared helpers ---

    def _require_indexed():
        """Check that a codebase is indexed and graph engine is available.

        Returns (state, None) on success, or (None, error_dict) on failure.
        """
        from contextro_mcp.state import get_state

        state = get_state()
        if not state.is_indexed or not state.graph_engine:
            return None, {"error": "No codebase indexed. Run 'index' first."}
        return state, None

    def _serialize_node(
        node: UniversalNode, codebase_path: Optional[Path] = None
    ) -> dict[str, Any]:
        """Convert a UniversalNode to a compact JSON-serializable dict.

        Omits None/empty/default values to reduce token usage.
        """
        file_path = node.location.file_path
        if codebase_path:
            try:
                file_path = str(Path(file_path).relative_to(codebase_path))
            except ValueError:
                pass

        result: dict[str, Any] = {
            # id omitted — internal graph ID, not useful to agents
            # compact keys: n=name, t=type, f=file, l=line
            "n": node.name,
            "f": file_path,
            "l": node.location.start_line,
        }
        # type omitted when function (most common case)
        if node.node_type.value != "function":
            result["t"] = node.node_type.value
        # Only include non-default/non-empty optional fields
        if node.language and node.language != "python":
            result["lang"] = node.language
        if node.complexity and node.complexity > 1:
            result["complexity"] = node.complexity
        if node.line_count and node.line_count > 1:
            result["lc"] = node.line_count
        elif node.location.end_line and node.location.end_line > node.location.start_line:
            # Only include end_line when line_count is unavailable
            result["el"] = node.location.end_line
        if node.docstring:
            # Truncate docstrings aggressively — first sentence is enough
            doc = node.docstring.split(".")[0].strip()
            if len(doc) > 100:
                doc = doc[:100] + "…"
            result["doc"] = doc
        if node.visibility and node.visibility != "public":
            result["vis"] = node.visibility
        if node.is_async:
            result["async"] = True
        if node.return_type:
            result["ret"] = node.return_type
        if node.parameter_types:
            result["params"] = node.parameter_types
        return _apply_disclosure(
            result,
            tool_name="overview",
            preview_keys=[
                "project_path",
                "total_files",
                "total_symbols",
                "total_relationships",
                "vector_chunks",
            ],
            max_list_items=8,
        )

    def _serialize_node_compact(node: UniversalNode, codebase_path: Optional[Path] = None) -> str:
        """Ultra-compact node serialization for list contexts (callers/callees).

        Returns 'name (file:line)' — ~30 tokens vs ~100 for full dict.
        Used in explain callers/callees and find_callers/find_callees.
        """
        fp = node.location.file_path
        if codebase_path:
            try:
                fp = str(Path(fp).relative_to(codebase_path))
            except ValueError:
                pass
        return f"{node.name} ({fp}:{node.location.start_line})"

    def _serialize_relationship(rel: UniversalRelationship) -> dict[str, Any]:
        """Convert a UniversalRelationship to a compact string.

        Format: "type:source→target" to minimize tokens.
        """
        result: dict[str, Any] = {"type": rel.relationship_type.value}
        if rel.source_id:
            result["src"] = rel.source_id
        if rel.target_id:
            result["tgt"] = rel.target_id
        # Only include strength if non-default
        if rel.strength and rel.strength != 1.0:
            result["w"] = rel.strength
        return result

    def _resolve_symbol(graph, name: str, exact: bool = True) -> List[UniversalNode]:
        """Find nodes by name in the graph."""
        return graph.find_nodes_by_name(name, exact=exact)

    def _filter_location_list(items: list, filter_path: str) -> list:
        """Filter a list of dicts by location string prefix."""
        return [item for item in items if item.get("location", "").startswith(filter_path)]

    def _relativize_location_str(location: str, root: Path) -> str:
        """Make a 'filepath:line' location string relative to root."""
        if ":" in location:
            fp, rest = location.rsplit(":", 1)
            try:
                fp = str(Path(fp).relative_to(root))
            except ValueError:
                pass
            return f"{fp}:{rest}"
        return location

    # --- MCP Tools ---

    @mcp.tool(annotations={"readOnlyHint": True})
    def status() -> dict[str, Any]:
        """Get Contextro server status including indexing stats."""
        guard_err = _guard("status")
        if guard_err:
            return guard_err

        from contextro_mcp.config import get_settings as _get_status_settings
        from contextro_mcp.state import get_state

        state = get_state()
        _status_settings = _get_status_settings()
        indexed = state.is_indexed
        # Memory monitoring via peak RSS (ru_maxrss = high-water mark)
        rss_raw = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
        # macOS returns bytes, Linux returns KB
        if sys.platform == "darwin":
            peak_rss_mb = rss_raw / (1024 * 1024)
        else:
            peak_rss_mb = rss_raw / 1024

        result: dict[str, Any] = {
            # indexed omitted when true — presence of codebase_path implies it
            "codebase_path": state.codebase_path.name if state.codebase_path else None,
        }
        if not indexed:
            result["indexed"] = False
        # Only surface memory when approaching the limit — saves 8 tokens on normal calls
        if peak_rss_mb > 300:
            result["memory"] = {"peak_rss_mb": round(peak_rss_mb, 1)}

        # Surface background indexing state
        with _index_job_lock:
            job_status = _index_job.get("status")
            job_result = dict(_index_job.get("result") or {})
        is_background_indexing = job_status == "indexing"
        if is_background_indexing and not indexed:
            result["indexing"] = True
            result["hint"] = "Indexing in progress — call status() again in a few seconds."
        elif job_status == "error":
            with _index_job_lock:
                result["index_error"] = _index_job.get("error", "Unknown error")
        elif job_status == "done" and not indexed:
            result["hint"] = "Index job completed but state not yet loaded. Call status() again."

        if indexed:
            cached_index_stats = (
                state.index_snapshot
                if _status_settings.status_use_cached_index_stats and hasattr(state, "index_snapshot")
                else {}
            )
            if not is_background_indexing:
                if state.vector_engine:
                    result["vector_chunks"] = cached_index_stats.get(
                        "vector_chunks", state.vector_engine.count()
                    )
                # bm25_fts_ready omitted — always true when indexed, agents don't need it
                if state.graph_engine:
                    full_graph_stats = cached_index_stats.get(
                        "graph", state.graph_engine.get_statistics()
                    )
                    # Single-char keys for maximum compactness
                    result["graph"] = {
                        "n": full_graph_stats.get("total_nodes", 0),
                        "r": full_graph_stats.get("total_relationships", 0),
                        "f": full_graph_stats.get("total_files", 0),
                    }
            else:
                result.setdefault(
                    "hint",
                    "Background indexing is finalizing. Search is available; detailed stats will appear when complete.",
                )

            # Git integration info
            if state.current_branch:
                result["branch"] = state.current_branch
            if state.current_head:
                result["head"] = state.current_head[:12]
            if state.branch_watcher and state.branch_watcher.is_running:
                result["realtime_watching"] = True
            if not is_background_indexing:
                commit_count = job_result.get("commits_indexed")
                if commit_count is not None:
                    if commit_count > 0:
                        result["commits_indexed"] = commit_count
                elif cached_index_stats.get("commits_indexed") is not None:
                    cc = cached_index_stats["commits_indexed"]
                    if cc > 0:
                        result["commits_indexed"] = cc
                else:
                    try:
                        from contextro_mcp.config import get_settings as _gs
                        from contextro_mcp.git.commit_indexer import CommitHistoryIndexer as _CI

                        _s = _gs()
                        cc = _CI.count_commits_in_db(str(_s.lancedb_path))
                        if cc > 0:
                            result["commits_indexed"] = cc
                    except Exception:
                        pass
            if state.cross_repo_manager and state.cross_repo_manager.repo_count > 1:
                result["cross_repo_count"] = state.cross_repo_manager.repo_count

            # Cache stats — helps agents know when cache is warm
            if hasattr(state, "_query_cache") and state._query_cache:
                cache = state._query_cache
                total = cache.hits + cache.misses
                if total > 0:
                    result["cache"] = {
                        "hits": cache.hits,
                        "misses": cache.misses,
                        "hit_rate": round(cache.hit_rate, 3),
                        "size": cache.size,
                    }
            if hasattr(state, "_session_tracker") and state._session_tracker is not None:
                edit_metrics = state._session_tracker.edit_metrics
                if any(edit_metrics.values()):
                    result["edit"] = edit_metrics

        elif job_status != "indexing":
            result.setdefault("hint", "Run 'index' first.")

        # Non-blocking update check — only shown when a newer version is available
        update = _check_for_update()
        if update:
            result["update_available"] = update
            result["update_hint"] = f"pip install --upgrade contextro  # {update} available"

        return _tool_result(result)

    @mcp.tool(annotations={"readOnlyHint": True})
    def health() -> dict[str, Any]:
        """Health check for readiness/liveness probes.

        Returns:
            Server health status including uptime and engine availability.
        """
        guard_err = _guard("health")
        if guard_err:
            return guard_err

        import time

        from contextro_mcp.state import get_state

        state = get_state()
        uptime = time.time() - state.started_at

        return _tool_result(
            {
                "status": "healthy",
                "uptime_seconds": round(uptime, 1),
                "indexed": state.is_indexed,
                "engines": {
                    "vector": state.vector_engine is not None,
                    "bm25": state.bm25_engine is not None,
                    "graph": state.graph_engine is not None,
                    "memory": state.memory_store is not None,
                },
            }
        )

    @mcp.tool()
    def index(
        path: Annotated[str, "Absolute path to the codebase directory (or comma-separated paths)"],
        paths: Annotated[str, "Additional comma-separated paths to index"] = "",
    ) -> dict[str, Any]:
        """Index a codebase into vector + graph engines.

        Indexing runs in the background to avoid MCP client timeouts.
        Returns immediately with status='indexing'. Call status() to check
        when indexing is complete (indexed: true).

        For incremental reindex (files changed since last index), this returns
        in <1s. For a fresh full index of a large codebase (10k+ files), it
        may take 20-60s in the background — use status() to poll for completion.

        Supports indexing multiple directories folder-by-folder. Each folder
        is processed sequentially (discover → parse → embed → store) and
        results are merged into shared engines.
        """
        import time as _time

        from contextro_mcp.config import get_settings
        from contextro_mcp.indexing.pipeline import IndexingPipeline
        from contextro_mcp.state import get_state

        global _pipeline, _index_job

        guard_err = _guard("index")
        if guard_err:
            return guard_err

        # Collect all paths from both parameters
        raw_paths = [p.strip() for p in path.split(",") if p.strip()]
        if paths:
            raw_paths.extend(p.strip() for p in paths.split(",") if p.strip())

        # Validate each path
        validated: list[Path] = []
        for raw in raw_paths:
            resolved, err = _validate_path(raw)
            if err:
                return {"error": f"Invalid path '{raw}': {err['error']}"}
            validated.append(resolved)

        if not validated:
            return {"error": "No valid paths provided."}

        # Use per-project storage unless CTX_STORAGE_DIR is explicitly set
        # This ensures each project gets its own isolated index
        from contextro_mcp.config import project_storage_dir, reset_settings

        if not os.environ.get("CTX_STORAGE_DIR"):
            primary_path = str(validated[0])
            project_dir = project_storage_dir(primary_path)
            project_dir.mkdir(parents=True, exist_ok=True)
            os.environ["CTX_STORAGE_DIR"] = str(project_dir)
            reset_settings()  # reload settings with new storage dir

        settings = get_settings()

        with _pipeline_lock:
            if _pipeline is None:
                _pipeline = IndexingPipeline(settings)
            elif (
                getattr(_pipeline, "settings", None) is not None
                and _pipeline.settings.storage_dir != settings.storage_dir
            ):
                # Storage dir changed (new project) — create fresh pipeline
                _pipeline = IndexingPipeline(settings)

        # Check if already indexing
        with _index_job_lock:
            if _index_job.get("status") == "indexing":
                return {
                    "status": "indexing",
                    "message": "Indexing already in progress. Call status() to check completion.",
                }

        # --- Fast path: incremental reindex (usually <1s, safe to run inline) ---
        metadata_path = settings.storage_path / "index_metadata.json"
        is_incremental = len(validated) == 1 and metadata_path.exists()

        if is_incremental:
            # Incremental is fast enough to run synchronously
            codebase_path = validated[0]
            try:
                previous_branch = state.current_branch if (state := get_state()) else None
                previous_head = state.current_head
                result = _pipeline.incremental_index(codebase_path)
                state.codebase_path = codebase_path
                state.codebase_paths = [codebase_path]
                state.vector_engine = _pipeline.vector_engine
                state.bm25_engine = _pipeline.bm25_engine
                state.graph_engine = _pipeline.graph_engine
                if hasattr(state, "_query_cache") and state._query_cache:
                    state._query_cache.invalidate()
                state.clear_derived_caches()
                index_result = result.to_dict()
                index_result["status"] = "done"
                index_result.pop("time_seconds", None)  # rarely needed by agents
                graph_files = state.graph_engine.get_statistics().get(
                    "total_files", 0
                ) if state.graph_engine else 0
                # Only include graph_files when it differs from total_files
                if graph_files != index_result.get("total_files", 0):
                    index_result["graph_files"] = graph_files
                # Strip zero-value noise fields
                for _zf in ("parse_errors", "files_added", "files_modified", "files_deleted"):
                    if not index_result.get(_zf):
                        index_result.pop(_zf, None)
                # Always strip redundant graph detail
                index_result.pop("graph_nodes", None)
                index_result.pop("graph_relationships", None)
                # Inline git integration for incremental path
                from contextro_mcp.git.commit_indexer import (
                    get_current_branch,
                    get_current_head,
                    is_git_repo,
                )

                if is_git_repo(str(codebase_path)):
                    state.current_branch = get_current_branch(str(codebase_path))
                    state.current_head = get_current_head(str(codebase_path))
                    index_result["branch"] = state.current_branch
                    fast_path_enabled = settings.incremental_index_fast_path_enabled
                    head_changed = (
                        state.current_branch != previous_branch or state.current_head != previous_head
                    )
                    files_changed = bool(
                        result.files_added or result.files_modified or result.files_deleted
                    )
                    if settings.commit_history_enabled:
                        if (not fast_path_enabled) or head_changed or files_changed:
                            try:
                                indexer = _get_commit_indexer()
                                cr = indexer.index_commits(
                                    repo_path=str(codebase_path),
                                    db_path=str(settings.lancedb_path),
                                    limit=settings.commit_history_limit,
                                    since=settings.commit_history_since or None,
                                    include_diffs=settings.commit_include_diffs,
                                )
                                index_result["commits_indexed"] = cr.get("total_commits", 0)
                            except Exception as eg:
                                logger.warning("Commit indexing failed: %s", eg)
                        elif state.index_snapshot.get("commits_indexed") is not None:
                            index_result["commits_indexed"] = state.index_snapshot["commits_indexed"]
                    if settings.realtime_indexing_enabled:
                        try:
                            watcher = _get_branch_watcher()
                            watcher.add_repo(str(codebase_path))
                            if not watcher.is_running:
                                watcher.start()
                            index_result["realtime_watching"] = True
                        except Exception as ew:
                            logger.warning("Branch watcher failed: %s", ew)
                _warm_search_pipeline(state, settings)
                state.capture_index_snapshot(
                    commits_indexed=index_result.get("commits_indexed")
                )
                with _index_job_lock:
                    _index_job = {"status": "done", "result": index_result}
                _get_tracker().track_index(str(codebase_path), index_result.get("total_chunks", 0))
                return index_result
            except Exception as e:
                logger.warning("Incremental index failed, falling back to full index: %s", e)
                # Fall through to background full index

        # --- Slow path: full index — run in background thread ---
        def _do_git_integration(vpath: Path, result: Any, index_result: dict) -> None:
            """Run git integration for one path after indexing."""
            from contextro_mcp.git.commit_indexer import (
                get_current_branch,
                get_current_head,
                is_git_repo,
            )

            if not is_git_repo(str(vpath)):
                return
            state = get_state()
            state.current_branch = get_current_branch(str(vpath))
            state.current_head = get_current_head(str(vpath))
            index_result["branch"] = state.current_branch

            if settings.commit_history_enabled:
                try:
                    indexer = _get_commit_indexer()
                    commit_result = indexer.index_commits(
                        repo_path=str(vpath),
                        db_path=str(settings.lancedb_path),
                        limit=settings.commit_history_limit,
                        since=settings.commit_history_since or None,
                        include_diffs=settings.commit_include_diffs,
                    )
                    index_result["commits_indexed"] = commit_result.get("total_commits", 0)
                except Exception as e:
                    logger.warning("Commit history indexing failed: %s", e)
                    index_result["commits_indexed"] = 0

            if settings.cross_repo_enabled:
                try:
                    manager = _get_cross_repo_manager()
                    manager.register_repo(str(vpath))
                    manager.update_repo_stats(
                        str(vpath),
                        total_files=getattr(result, "total_files", 0),
                        total_symbols=getattr(result, "total_symbols", 0),
                        total_chunks=getattr(result, "total_chunks", 0),
                        total_commits=index_result.get("commits_indexed", 0),
                    )
                except Exception as e:
                    logger.warning("Cross-repo registration failed: %s", e)

            if settings.realtime_indexing_enabled:
                try:
                    watcher = _get_branch_watcher()
                    watcher.add_repo(str(vpath))
                    if not watcher.is_running:
                        watcher.start()
                    index_result["realtime_watching"] = True
                except Exception as e:
                    logger.warning("Failed to start branch watcher: %s", e)

        def _run_index():
            try:
                with _index_job_lock:
                    _index_job["status"] = "indexing"
                    _index_job["started_at"] = _time.time()

                state = get_state()

                if len(validated) > 1:
                    result = _pipeline.multi_index(validated)
                    state.codebase_path = validated[0]
                    state.codebase_paths = validated
                else:
                    result = _pipeline.index(validated[0])
                    state.codebase_path = validated[0]
                    state.codebase_paths = [validated[0]]

                state.vector_engine = _pipeline.vector_engine
                state.bm25_engine = _pipeline.bm25_engine
                state.graph_engine = _pipeline.graph_engine
                if hasattr(state, "_query_cache") and state._query_cache:
                    state._query_cache.invalidate()
                state.clear_derived_caches()

                index_result = result.to_dict()
                index_result["status"] = "done"
                index_result.pop("time_seconds", None)  # rarely needed by agents
                # Only include graph_files when it differs from total_files
                graph_files = state.graph_engine.get_statistics().get(
                    "total_files", 0
                ) if state.graph_engine else 0
                if graph_files != index_result.get("total_files", 0):
                    index_result["graph_files"] = graph_files
                # Strip zero-value noise fields
                for _zf in ("parse_errors", "files_added", "files_modified", "files_deleted"):
                    if not index_result.get(_zf):
                        index_result.pop(_zf, None)
                # Always strip redundant graph detail (graph_files covers it)
                index_result.pop("graph_nodes", None)
                index_result.pop("graph_relationships", None)

                for vpath in validated:
                    _do_git_integration(vpath, result, index_result)

                _warm_search_pipeline(state, settings)
                state.capture_index_snapshot(commits_indexed=index_result.get("commits_indexed"))
                logger.info("Search pipeline pre-warmed")

                elapsed = round(_time.time() - _index_job.get("started_at", _time.time()), 3)
                # time_seconds already popped above — don't re-add it
                with _index_job_lock:
                    _index_job["status"] = "done"
                    _index_job["result"] = index_result
                _get_tracker().track_index(str(validated[0]), index_result.get("total_chunks", 0))
                logger.info("Background indexing complete in %ss", elapsed)

            except Exception as e:
                logger.error("Background indexing failed: %s", e)
                with _index_job_lock:
                    _index_job["status"] = "error"
                    _index_job["error"] = str(e)

        with _index_job_lock:
            _index_job = {"status": "indexing", "started_at": _time.time()}

        t = threading.Thread(target=_run_index, daemon=True, name="contextro-indexer")
        t.start()

        return {
            "status": "indexing",
            "message": (
                "Indexing started in background. "
                "Call status() in a few seconds to check when indexed: true. "
                "For most codebases this takes 10-30s."
            ),
            "path": str(validated[0]),
        }

    @mcp.tool(annotations={"readOnlyHint": True})
    def search(
        query: Annotated[str, "Natural language or code query (e.g. 'retry logic')"],
        limit: Annotated[int, "Max results (default 10, max 100)"] = 10,
        language: Annotated[str, "Filter by language (e.g. 'python')"] = "",
        symbol_type: Annotated[str, "Filter by type (e.g. 'function', 'class')"] = "",
        mode: Annotated[str, "Search mode: 'hybrid', 'vector', or 'bm25'"] = "hybrid",
        rerank: Annotated[bool, "FlashRank reranking (default True)"] = True,
        live_grep: Annotated[bool, "Force live-grep fallback (rg/grep)"] = False,
        context_budget: Annotated[
            int,
            "Max tokens for results (0=unlimited). Dynamically adjusts result count and snippet length.",
        ] = 0,
    ) -> dict[str, Any]:
        """PREFERRED over Grep/Glob. Semantic search with code snippets."""
        guard_err = _guard("search")
        if guard_err:
            return guard_err

        from contextro_mcp.config import get_settings
        from contextro_mcp.execution.runtime import build_search_runtime
        from contextro_mcp.execution.search import SearchExecutionEngine, SearchExecutionOptions
        from contextro_mcp.state import get_state

        err = _validate_query(query)
        if err:
            return err

        state = get_state()
        if not state.is_indexed or not state.vector_engine:
            return {"error": "No codebase indexed. Run 'index' first."}

        settings = get_settings()
        runtime = build_search_runtime(state, settings)
        engine = SearchExecutionEngine(runtime)
        return _tool_result(
            engine.execute(
                SearchExecutionOptions(
                    query=query,
                    limit=limit,
                    language=language,
                    symbol_type=symbol_type,
                    mode=mode,
                    rerank=rerank,
                    live_grep=live_grep,
                    context_budget=context_budget,
                )
            ),
            fmt="search",
        )

    @mcp.tool(annotations={"readOnlyHint": True})
    def find_symbol(
        name: Annotated[str, "Symbol name (e.g. 'create_server', 'TokenBudget')"],
        exact: Annotated[bool, "True for exact match, False for fuzzy substring"] = True,
    ) -> dict[str, Any]:
        """PREFERRED over Grep for finding symbol definitions."""
        guard_err = _guard("find_symbol")
        if guard_err:
            return guard_err

        err = _validate_symbol_name(name)
        if err:
            return err

        state, err = _require_indexed()
        if err:
            return err

        matches = _resolve_symbol(state.graph_engine, name, exact=exact)
        if not matches:
            msg = f"Symbol '{name}' not found."
            if exact:
                msg += " Try exact=False for fuzzy matching."
            return {"error": msg}

        # Cap fuzzy results to prevent 94KB+ responses
        MAX_SYMBOLS = 20
        total_matches = len(matches)
        if len(matches) > MAX_SYMBOLS:
            matches = matches[:MAX_SYMBOLS]

        # Only traverse caller/callee graph when result set is small (exact or near-exact match).
        # For large fuzzy result sets the traversal adds significant payload with little value —
        # the agent should use find_callers/find_callees on the specific symbol instead.
        include_graph = total_matches <= 5

        symbols = []
        for node in matches:
            entry = _serialize_node(node, state.codebase_path)
            if include_graph:
                callers = state.graph_engine.get_callers(node.id)
                callees = state.graph_engine.get_callees(node.id)
                if callers:
                    # callers_count omitted — len(top_callers) tells the agent
                    entry["callers"] = [c.name for c in callers[:5]]
                    if len(callers) > 5:
                        entry["callers+"] = len(callers)
                if callees:
                    entry["callees"] = [c.name for c in callees[:5]]
                    if len(callees) > 5:
                        entry["callees+"] = len(callees)
            symbols.append(entry)

        # Single result: return symbol directly (no wrapper)
        # Multiple results: return {total: N, symbols: [...]}
        if len(symbols) == 1:
            result = symbols[0]
        else:
            result = {"total": total_matches, "symbols": symbols}
        if total_matches > MAX_SYMBOLS:
            result["note"] = (
                f"Showing top {MAX_SYMBOLS} of {total_matches}. Use exact=True for precise lookup."
            )
        elif total_matches > 1:
            result["disambiguation_note"] = (
                f"{total_matches} definitions found. Use find_callees/find_callers with the "
                "specific file path to avoid mixing results from different implementations."
            )
        _get_tracker().track_find_symbol(name, found=True)
        return _tool_result(result, fmt="symbols")

    @mcp.tool(annotations={"readOnlyHint": True})
    def find_callers(
        symbol_name: Annotated[str, "Name of the function to find callers for"],
    ) -> dict[str, Any]:
        """Find all functions that call a given symbol. More accurate than Grep."""
        guard_err = _guard("find_callers")
        if guard_err:
            return guard_err

        err = _validate_symbol_name(symbol_name)
        if err:
            return err

        state, err = _require_indexed()
        if err:
            return err

        matches = _resolve_symbol(state.graph_engine, symbol_name, exact=True)
        if not matches:
            return {"error": f"Symbol '{symbol_name}' not found."}

        # When multiple definitions share the same name, group callers per-definition.
        if len(matches) > 1:
            definitions = []
            total = 0
            for node in matches:
                node_callers = [
                    _serialize_node_compact(c, state.codebase_path)
                    for c in state.graph_engine.get_callers(node.id)
                ]
                total += len(node_callers)
                file_path = node.location.file_path
                if state.codebase_path:
                    try:
                        file_path = str(Path(file_path).relative_to(state.codebase_path))
                    except ValueError:
                        pass
                definitions.append({
                    "definition": f"{file_path}:{node.location.start_line}",
                    "callers": node_callers[:20],
                })
            _get_tracker().track_find_symbol(symbol_name, found=bool(total))
            return _tool_result(
                _apply_disclosure(
                    {
                        "symbol": symbol_name,
                        "total": total,
                        "disambiguation_note": (
                            f"{len(matches)} definitions found; callers grouped per definition."
                        ),
                        "definitions": definitions,
                    },
                    tool_name="find_callers",
                    preview_keys=["symbol", "total", "disambiguation_note"],
                )
            )

        all_callers = []
        seen: set[str] = set()
        for node in matches:
            for caller in state.graph_engine.get_callers(node.id):
                if caller.id not in seen:
                    seen.add(caller.id)
                    all_callers.append(_serialize_node_compact(caller, state.codebase_path))

        # Cap at 20 callers to keep output manageable
        total = len(all_callers)
        all_callers = all_callers[:20]

        _get_tracker().track_find_symbol(symbol_name, found=bool(all_callers))
        # Nano format: callers list only, no redundant total/wrapper
        return _tool_result({"callers": all_callers})

    @mcp.tool(annotations={"readOnlyHint": True})
    def find_callees(
        symbol_name: Annotated[str, "Name of the function to find callees for"],
    ) -> dict[str, Any]:
        """Find all functions called by a given function. Traces execution flow."""
        guard_err = _guard("find_callees")
        if guard_err:
            return guard_err

        err = _validate_symbol_name(symbol_name)
        if err:
            return err

        state, err = _require_indexed()
        if err:
            return err

        matches = _resolve_symbol(state.graph_engine, symbol_name, exact=True)
        if not matches:
            return {"error": f"Symbol '{symbol_name}' not found."}

        # When multiple definitions share the same name, group callees per-definition
        # so the agent can see which callees belong to which implementation.
        if len(matches) > 1:
            definitions = []
            total = 0
            for node in matches:
                node_callees = [
                    _serialize_node_compact(c, state.codebase_path)
                    for c in state.graph_engine.get_callees(node.id)
                ]
                total += len(node_callees)
                file_path = node.location.file_path
                if state.codebase_path:
                    try:
                        file_path = str(Path(file_path).relative_to(state.codebase_path))
                    except ValueError:
                        pass
                definitions.append({
                    "definition": f"{file_path}:{node.location.start_line}",
                    "callees": node_callees[:20],
                })
            _get_tracker().track_find_symbol(symbol_name, found=bool(total))
            return _tool_result(
                _apply_disclosure(
                    {
                        "symbol": symbol_name,
                        "total": total,
                        "disambiguation_note": (
                            f"{len(matches)} definitions found; callees grouped per definition."
                        ),
                        "definitions": definitions,
                    },
                    tool_name="find_callees",
                    preview_keys=["symbol", "total", "disambiguation_note"],
                )
            )

        all_callees = []
        seen: set[str] = set()
        for node in matches:
            for callee in state.graph_engine.get_callees(node.id):
                if callee.id not in seen:
                    seen.add(callee.id)
                    all_callees.append(_serialize_node_compact(callee, state.codebase_path))

        total = len(all_callees)
        all_callees = all_callees[:20]

        _get_tracker().track_find_symbol(symbol_name, found=bool(all_callees))
        return _tool_result({"callees": all_callees})

    @mcp.tool(annotations={"readOnlyHint": True})
    def analyze(
        path: Annotated[
            str, "Optional relative path to filter analysis (subdirectory or file)"
        ] = "",
    ) -> dict[str, Any]:
        """Run code analysis: complexity, dependencies, smells, quality score."""
        guard_err = _guard("analyze")
        if guard_err:
            return guard_err

        state, err = _require_indexed()
        if err:
            return err

        from contextro_mcp.analysis.code_analyzer import CodeAnalyzer

        analyzer = CodeAnalyzer(state.graph_engine)

        result = {
            "complexity": analyzer.analyze_complexity(),
            "code_smells": analyzer.detect_code_smells(),
            "quality": analyzer.calculate_quality_metrics(),
        }

        root = state.codebase_path

        # Filter by path if specified
        if path and root:
            filter_path = str(root / path)

            # Filter complexity high_complexity_functions
            if "high_complexity_functions" in result["complexity"]:
                result["complexity"]["high_complexity_functions"] = _filter_location_list(
                    result["complexity"]["high_complexity_functions"], filter_path
                )

            # Filter code smells
            for smell_key in ["long_functions", "complex_functions", "large_classes", "dead_code"]:
                if smell_key in result["code_smells"]:
                    result["code_smells"][smell_key] = _filter_location_list(
                        result["code_smells"][smell_key], filter_path
                    )

        # Cap all lists to prevent token bloat on large repos
        MAX_ITEMS = 10
        if "high_complexity_functions" in result["complexity"]:
            items = result["complexity"]["high_complexity_functions"]
            if len(items) > MAX_ITEMS:
                result["complexity"]["high_complexity_functions"] = items[:MAX_ITEMS]
                result["complexity"]["high_complexity_total"] = len(items)

        for smell_key in ["long_functions", "complex_functions", "large_classes", "dead_code"]:
            if smell_key in result["code_smells"]:
                items = result["code_smells"][smell_key]
                if len(items) > MAX_ITEMS:
                    result["code_smells"][f"{smell_key}_total"] = len(items)
                    result["code_smells"][smell_key] = items[:MAX_ITEMS]

        # Relativize paths in results
        if root:
            # Complexity
            for item in result["complexity"].get("high_complexity_functions", []):
                if "location" in item:
                    item["location"] = _relativize_location_str(item["location"], root)

            # Code smells
            for smell_key in ["long_functions", "complex_functions", "large_classes", "dead_code"]:
                for item in result["code_smells"].get(smell_key, []):
                    if "location" in item:
                        item["location"] = _relativize_location_str(item["location"], root)

        return _apply_disclosure(result, tool_name="analyze", preview_keys=["quality"])

    @mcp.tool(annotations={"readOnlyHint": True})
    def impact(
        symbol_name: Annotated[str, "Name of the function to analyze impact for"],
        max_depth: Annotated[int, "Max depth of transitive caller traversal (default 10)"] = 10,
    ) -> dict[str, Any]:
        """MUST use before refactoring. Change impact analysis."""
        guard_err = _guard("impact")
        if guard_err:
            return guard_err

        err = _validate_symbol_name(symbol_name)
        if err:
            return err

        state, err = _require_indexed()
        if err:
            return err

        max_depth = max(1, min(max_depth, 50))

        matches = _resolve_symbol(state.graph_engine, symbol_name, exact=True)
        if not matches:
            return {"error": f"Symbol '{symbol_name}' not found."}

        all_impacted = []
        seen: set[str] = set()
        for node in matches:
            for caller in state.graph_engine.get_transitive_callers(node.id, max_depth=max_depth):
                if caller.id not in seen:
                    seen.add(caller.id)
                    all_impacted.append(_serialize_node_compact(caller, state.codebase_path))

        # Cap at 20 impacted symbols to keep output manageable
        total = len(all_impacted)
        all_impacted = all_impacted[:20]

        # Group by file and directory — parse compact "name (file:line)" strings
        by_dir: dict[str, list[str]] = {}
        by_file: dict[str, list[str]] = {}
        for item in all_impacted:
            # Parse "name (file:line)" format
            if " (" in item and item.endswith(")"):
                name_part = item.split(" (")[0]
                loc_part = item[item.index("(") + 1 : -1]  # "file:line"
                fp = loc_part.rsplit(":", 1)[0] if ":" in loc_part else loc_part
            else:
                name_part = item
                fp = ""
            if fp not in by_file:
                by_file[fp] = []
            by_file[fp].append(name_part)
            dir_part = str(Path(fp).parent) if fp else "."
            if dir_part not in by_dir:
                by_dir[dir_part] = []
            by_dir[dir_part].append(fp.rsplit("/", 1)[-1] if "/" in fp else fp)

        by_dir = {d: list(dict.fromkeys(files)) for d, files in by_dir.items()}

        _get_tracker().track_impact(symbol_name, total)
        return _apply_disclosure(
            {
                "symbol": symbol_name,
                "max_depth": max_depth,
                "total_impacted": total,
                "impacted_symbols": all_impacted,
                "impacted_files": by_file,
                "impacted_dirs": by_dir,
            },
            tool_name="impact",
            preview_keys=["symbol", "max_depth", "total_impacted"],
        )

    @mcp.tool(annotations={"readOnlyHint": True})
    def explain(
        symbol_name: Annotated[str, "Name of the symbol to explain"],
        verbosity: Annotated[
            str, "Output detail level: 'summary', 'detailed', or 'full'"
        ] = "detailed",
    ) -> dict[str, Any]:
        """PREFERRED over Read for understanding code symbols."""
        guard_err = _guard("explain")
        if guard_err:
            return guard_err

        from contextro_mcp.formatting.response_builder import ResponseBuilder
        from contextro_mcp.formatting.token_budget import TokenBudget

        err = _validate_symbol_name(symbol_name)
        if err:
            return err

        if verbosity not in TokenBudget.BUDGETS:
            valid = list(TokenBudget.BUDGETS.keys())
            return {"error": f"Invalid verbosity: {verbosity}. Must be one of {valid}"}

        state, err = _require_indexed()
        if err:
            return err

        matches = _resolve_symbol(state.graph_engine, symbol_name, exact=True)
        if not matches:
            matches = _resolve_symbol(state.graph_engine, symbol_name, exact=False)
        if not matches:
            return {"error": f"Symbol '{symbol_name}' not found."}

        node = matches[0]
        symbol_data = _serialize_node(node, state.codebase_path)

        # Relationships — cap to prevent token bloat on widely-used symbols
        MAX_CALLERS = 15
        MAX_CALLEES = 15
        all_callers = state.graph_engine.get_callers(node.id)
        all_callees = state.graph_engine.get_callees(node.id)
        # Use compact format: "name (file:line)" — 3x fewer tokens than full dicts
        callers = [
            _serialize_node_compact(c, state.codebase_path) for c in all_callers[:MAX_CALLERS]
        ]
        callees = [
            _serialize_node_compact(c, state.codebase_path) for c in all_callees[:MAX_CALLEES]
        ]
        if callers:
            symbol_data["callers"] = callers
            if len(all_callers) > MAX_CALLERS:
                symbol_data["callers_total"] = len(all_callers)
                symbol_data["callers_note"] = (
                    f"Showing top {MAX_CALLERS} of {len(all_callers)}. "
                    "Use find_callers() for the full list."
                )
        if callees:
            symbol_data["callees"] = callees
            if len(all_callees) > MAX_CALLEES:
                symbol_data["callees_total"] = len(all_callees)
        # rels_in/rels_out contain raw graph IDs — only useful for full verbosity
        if verbosity == "full":
            rels_from = state.graph_engine.get_relationships_from(node.id)
            rels_to = state.graph_engine.get_relationships_to(node.id)
            if rels_from:
                symbol_data["rels_out"] = [_serialize_relationship(r) for r in rels_from[:20]]
                if len(rels_from) > 20:
                    symbol_data["rels_out_total"] = len(rels_from)
            if rels_to:
                symbol_data["rels_in"] = [_serialize_relationship(r) for r in rels_to[:20]]
                if len(rels_to) > 20:
                    symbol_data["rels_in_total"] = len(rels_to)

        # Vector search for related code (compact: name + file + score only)
        search_results = []
        if state.vector_engine:
            search_text = node.name
            if node.docstring:
                search_text += " " + node.docstring
            try:
                raw_results = state.vector_engine.search(search_text, limit=5)
                # Return compact references, not full code snippets
                for r in raw_results:
                    fp = r.get("filepath", "")
                    if fp and state.codebase_path:
                        try:
                            fp = str(Path(fp).relative_to(state.codebase_path))
                        except ValueError:
                            pass
                    search_results.append(
                        {
                            "name": r.get("symbol_name", ""),
                            "file": fp,
                        }
                    )
            except Exception as e:
                logger.debug("Vector search failed in explain: %s", e)
                pass

        # Code analysis — only include for 'full' verbosity to save tokens
        analysis = {}
        if verbosity == "full":
            try:
                from contextro_mcp.analysis.code_analyzer import CodeAnalyzer
                from contextro_mcp.core.graph_models import NodeType as _NT

                analyzer = CodeAnalyzer(state.graph_engine)
                quality = analyzer.calculate_quality_metrics()
                total_funcs = len(state.graph_engine.get_nodes_by_type(_NT.FUNCTION))
                analysis = {
                    "complexity": {"total_functions": total_funcs},
                    "quality": quality,
                }
            except Exception as e:
                logger.debug("Code analysis failed in explain: %s", e)
                pass

        builder = ResponseBuilder(verbosity)
        # For summary mode, strip verbose relationship data from symbol_data
        if verbosity == "summary":
            summary_data = {
                k: v
                for k, v in symbol_data.items()
                if k not in ("callers", "callees", "rels_in", "rels_out")
            }
            if "callers_total" in symbol_data:
                summary_data["callers_total"] = symbol_data["callers_total"]
            elif "callers" in symbol_data:
                summary_data["callers_total"] = len(symbol_data["callers"])
            if "callees" in symbol_data:
                summary_data["callees_total"] = len(symbol_data["callees"])
            _get_tracker().track_explain(symbol_name)
            return builder.build_explain_response(summary_data, search_results, analysis)
        _get_tracker().track_explain(symbol_name)
        result = builder.build_explain_response(symbol_data, search_results, analysis)
        return _apply_disclosure(
            result, tool_name="explain", preview_keys=["symbol", "type", "file", "line"]
        )

    @mcp.tool(annotations={"readOnlyHint": True})
    def overview() -> dict[str, Any]:
        """PREFERRED over Glob/ls for project exploration.

        Returns language breakdown, file count, symbol counts by type,
        code quality summary, top-level modules, and key statistics.

        Output is capped to prevent token bloat on large repos.
        For deeper exploration, use search() or architecture().

        Returns:
            Project overview with structure, languages, quality, and key symbols.
        """
        guard_err = _guard("overview")
        if guard_err:
            return guard_err

        state, err = _require_indexed()
        if err:
            return err

        from contextro_mcp.core.graph_models import NodeType

        graph = state.graph_engine
        stats = graph.get_statistics()

        # Language breakdown with file counts
        languages = {}
        for lang, count in stats.get("nodes_by_language", {}).items():
            languages[lang] = count

        # Directory summary: only top-level directories with counts
        # (avoids dumping hundreds of nested paths for large repos)
        directories_flat: dict[str, int] = {}
        root = state.codebase_path
        for fp in graph._file_nodes:
            try:
                rel = str(Path(fp).relative_to(root))
            except ValueError:
                rel = fp
            parts = Path(rel).parts
            # Group by top-level directory only
            top_dir = parts[0] if parts else "."
            directories_flat[top_dir] = directories_flat.get(top_dir, 0) + 1

        # Sort by file count descending, cap at 25 entries
        sorted_dirs = sorted(directories_flat.items(), key=lambda x: x[1], reverse=True)
        directories_summary = dict(sorted_dirs[:25])
        if len(sorted_dirs) > 25:
            directories_summary["...and_more"] = sum(c for _, c in sorted_dirs[25:])

        # Top-level modules (sorted by symbol count descending, capped at 15)
        modules = graph.get_nodes_by_type(NodeType.MODULE)
        module_summaries = []
        for mod in modules:
            rels = graph.get_relationships_from(mod.id)
            child_count = len(rels)
            mod_path = mod.location.file_path
            if root:
                try:
                    mod_path = str(Path(mod_path).relative_to(root))
                except ValueError:
                    pass
            module_summaries.append(
                {
                    "name": mod.name,
                    "file": mod_path,
                    "symbols": child_count,
                    "lines": mod.line_count,
                }
            )
        module_summaries.sort(key=lambda m: m["symbols"], reverse=True)

        # Chunk count
        chunk_count = 0
        if state.vector_engine:
            chunk_count = state.vector_engine.count()

        result: dict[str, Any] = {
            "project_path": root.name if root else None,
            "total_files": stats.get("total_files", 0),
            "total_symbols": stats.get("total_nodes", 0),
            "total_relationships": stats.get("total_relationships", 0),
            "vector_chunks": chunk_count,
            "symbols_by_type": stats.get("nodes_by_type", {}),
            "languages": languages,
            "directories": directories_summary,
        }
        # Only include top_modules if non-empty (TypeScript repos have 0 modules)
        if module_summaries:
            result["top_modules"] = module_summaries[:10]

        return _apply_disclosure(
            result,
            tool_name="overview",
            preview_keys=[
                "project_path",
                "total_files",
                "total_symbols",
                "total_relationships",
                "vector_chunks",
            ],
            max_list_items=8,
        )

    @mcp.tool(annotations={"readOnlyHint": True})
    def architecture() -> dict[str, Any]:
        """PREFERRED over manual browsing for project design.

        Analyzes directory structure as layers, class hierarchies, hub symbols,
        and entry points to produce a compact architectural summary.

        Returns:
            Architectural documentation with layers, dependencies,
            key abstractions, and structural insights.
        """
        guard_err = _guard("architecture")
        if guard_err:
            return guard_err

        state, err = _require_indexed()
        if err:
            return err

        from contextro_mcp.core.graph_models import NodeType

        graph = state.graph_engine
        root = state.codebase_path

        # Identify architectural layers from file paths (works for all languages)
        # Group by top-2 directory levels: "apps/app", "convex/modules", etc.
        layers: dict[str, dict[str, int]] = {}
        for fp in graph._file_nodes:
            try:
                rel = str(Path(fp).relative_to(root))
            except ValueError:
                rel = fp
            parts = Path(rel).parts
            if len(parts) >= 3:
                layer = str(Path(parts[0]) / parts[1])
            elif len(parts) >= 2:
                layer = parts[0]
            else:
                layer = "root"
            if layer not in layers:
                layers[layer] = {"files": 0, "symbols": 0}
            layers[layer]["files"] += 1

        # Count symbols per layer
        for node in graph.nodes.values():
            fp = node.location.file_path
            try:
                rel = str(Path(fp).relative_to(root))
            except ValueError:
                rel = fp
            parts = Path(rel).parts
            if len(parts) >= 3:
                layer = str(Path(parts[0]) / parts[1])
            elif len(parts) >= 2:
                layer = parts[0]
            else:
                layer = "root"
            if layer in layers:
                layers[layer]["symbols"] += 1

        # Top 15 layers by symbol count
        top_layers = dict(sorted(layers.items(), key=lambda x: x[1]["symbols"], reverse=True)[:15])

        # Top classes by connection count (methods/relationships)
        classes = graph.get_nodes_by_type(NodeType.CLASS)
        class_info = []
        for cls in classes[:200]:  # scan first 200 to avoid O(n) on huge repos
            in_deg, out_deg = graph.get_node_degree(cls.id)
            cls_path = cls.location.file_path
            if root:
                try:
                    cls_path = str(Path(cls_path).relative_to(root))
                except ValueError:
                    pass
            class_info.append(
                {
                    "name": cls.name,
                    "file": cls_path,
                    "connections": in_deg + out_deg,
                    "lines": cls.line_count or 0,
                }
            )
        class_info.sort(key=lambda c: c["connections"], reverse=True)

        # Entry points
        entry_patterns = {
            "main",
            "run",
            "start",
            "handler",
            "create_server",
            "app",
            "default",
            "middleware",
            "layout",
            "page",
        }
        functions = graph.get_nodes_by_type(NodeType.FUNCTION)
        entry_points = []
        for func in functions:
            if func.name in entry_patterns:
                fp = func.location.file_path
                if root:
                    try:
                        fp = str(Path(fp).relative_to(root))
                    except ValueError:
                        pass
                entry_points.append({"name": func.name, "file": fp})
        # Deduplicate by name, keep first 10
        seen_entries = set()
        unique_entries = []
        for ep in entry_points:
            key = ep["name"] + ":" + ep["file"]
            if key not in seen_entries:
                seen_entries.add(key)
                unique_entries.append(ep)
        entry_points = unique_entries[:10]

        # Hub symbols: highest degree (most connections)
        hub_symbols = []
        for node in graph.nodes.values():
            in_deg, out_deg = graph.get_node_degree(node.id)
            total_deg = in_deg + out_deg
            if total_deg >= 5:
                fp = node.location.file_path
                if root:
                    try:
                        fp = str(Path(fp).relative_to(root))
                    except ValueError:
                        pass
                hub_symbols.append(
                    {
                        "name": node.name,
                        "type": node.node_type.value,
                        "file": fp,
                        "connections": total_deg,
                    }
                )
        hub_symbols.sort(key=lambda h: h["connections"], reverse=True)

        stats = graph.get_statistics()

        return _apply_disclosure(
            {
                "total_files": stats.get("total_files", len(graph._file_nodes)),
                "total_symbols": stats.get("total_nodes", 0),
                "total_relationships": stats.get("total_relationships", 0),
                "layers": top_layers,
                "top_classes": class_info[:10],
                "entry_points": entry_points,
                "hub_symbols": hub_symbols[:10],
            },
            tool_name="architecture",
            preview_keys=["total_files", "total_symbols", "total_relationships"],
        )

    @mcp.tool(annotations={"readOnlyHint": True})
    def focus(
        path: Annotated[str, "Relative or absolute file path to focus on"],
        include_code: Annotated[
            bool, "Include a compressed code preview for the target file (default true)"
        ] = True,
    ) -> dict[str, Any]:
        """Low-token context slice for a single file."""
        guard_err = _guard("focus")
        if guard_err:
            return guard_err

        state, err = _require_indexed()
        if err:
            return err

        from contextro_mcp.reports.product import build_focus_report

        try:
            result = build_focus_report(state, path, include_code=include_code)
        except ValueError as e:
            return {"error": str(e)}

        return _apply_disclosure(
            result,
            tool_name="focus",
            preview_keys=["path", "role", "symbols", "imports", "nearby_tests", "blast_radius"],
            max_list_items=8,
        )

    @mcp.tool(annotations={"readOnlyHint": True})
    def restore() -> dict[str, Any]:
        """Project restore/context-bomb summary for quick repo re-entry."""
        guard_err = _guard("restore")
        if guard_err:
            return guard_err

        state, err = _require_indexed()
        if err:
            return err

        from contextro_mcp.reports.product import build_restore_report

        return _apply_disclosure(
            build_restore_report(state),
            tool_name="restore",
            preview_keys=["project", "entry_points", "layers", "risk_summary"],
            max_list_items=8,
        )

    @mcp.tool(annotations={"readOnlyHint": True})
    def audit() -> dict[str, Any]:
        """Packaged audit report combining quality, graph risk, and static analyses."""
        guard_err = _guard("audit")
        if guard_err:
            return guard_err

        state, err = _require_indexed()
        if err:
            return err

        from contextro_mcp.reports.product import build_audit_report

        return _apply_disclosure(
            build_audit_report(state),
            tool_name="audit",
            preview_keys=["summary", "recommendations"],
            max_list_items=8,
        )

    @mcp.tool(annotations={"readOnlyHint": True})
    def dead_code() -> dict[str, Any]:
        """Dedicated dead-code analysis based on entry-point reachability."""
        guard_err = _guard("dead_code")
        if guard_err:
            return guard_err

        state, err = _require_indexed()
        if err:
            return err

        from contextro_mcp.reports.product import get_static_analysis_bundle

        result = get_static_analysis_bundle(state)["dead_code"]

        # Detect Next.js/App Router and add a disclaimer — App Router pages/layouts are
        # loaded by the framework at runtime and are NOT reachable via static import tracing.
        # Dead-code results for these files are unreliable.
        is_nextjs = False
        if state.codebase_path:
            next_config = state.codebase_path / "next.config.js"
            next_config_ts = state.codebase_path / "next.config.ts"
            next_config_mjs = state.codebase_path / "next.config.mjs"
            if next_config.exists() or next_config_ts.exists() or next_config_mjs.exists():
                is_nextjs = True
            elif (state.codebase_path / "app").is_dir() or (state.codebase_path / "src" / "app").is_dir():
                # Heuristic: App Router directory present
                is_nextjs = True
        if is_nextjs:
            result["framework_warning"] = (
                "Next.js / App Router detected. Pages, layouts, route handlers, and middleware "
                "are loaded by the framework at runtime — they are NOT reachable via static import "
                "tracing. Files under app/ or pages/ flagged as 'unreachable' are likely false "
                "positives. Treat dead_code results for this repo as advisory only."
            )

        return _apply_disclosure(
            result,
            tool_name="dead_code",
            preview_keys=["summary", "framework_warning", "unused_files", "unused_symbols"],
            max_list_items=8,
        )

    @mcp.tool(annotations={"readOnlyHint": True})
    def circular_dependencies() -> dict[str, Any]:
        """Dedicated circular dependency analysis using SCCs."""
        guard_err = _guard("circular_dependencies")
        if guard_err:
            return guard_err

        state, err = _require_indexed()
        if err:
            return err

        from contextro_mcp.reports.product import get_static_analysis_bundle

        return _apply_disclosure(
            get_static_analysis_bundle(state)["circular_dependencies"],
            tool_name="circular_dependencies",
            preview_keys=["summary", "cycles"],
            max_list_items=8,
        )

    @mcp.tool(annotations={"readOnlyHint": True})
    def test_coverage_map() -> dict[str, Any]:
        """Dedicated static test coverage map."""
        guard_err = _guard("test_coverage_map")
        if guard_err:
            return guard_err

        state, err = _require_indexed()
        if err:
            return err

        from contextro_mcp.reports.product import get_static_analysis_bundle

        return _apply_disclosure(
            get_static_analysis_bundle(state)["test_coverage_map"],
            tool_name="test_coverage_map",
            preview_keys=["summary", "uncovered_files", "uncovered_symbols", "note"],
            max_list_items=8,
        )

    @mcp.tool()
    def sidecar_export(
        path: Annotated[str, "Optional file or directory to export sidecars for"] = "",
        include_code: Annotated[
            bool, "Include compressed code previews in each sidecar (default false)"
        ] = False,
        clean: Annotated[bool, "Remove Contextro-managed sidecars instead of generating them"] = False,
    ) -> dict[str, Any]:
        """Generate or clean file-adjacent `.graph.*` sidecars."""
        guard_err = _guard("sidecar_export")
        if guard_err:
            return guard_err

        state, err = _require_indexed()
        if err:
            return err

        from contextro_mcp.artifacts.sidecars import clean_sidecars, export_sidecars

        try:
            if clean:
                result = clean_sidecars(state, target_path=path or None)
            else:
                result = export_sidecars(state, target_path=path or None, include_code=include_code)
            return _apply_disclosure(
                result,
                tool_name="sidecar_export",
                preview_keys=["count"],
                max_list_items=8,
            )
        except ValueError as e:
            return {"error": str(e)}

    @mcp.tool()
    def skill_prompt(
        target_path: Annotated[
            str, "Optional file to update (e.g. CLAUDE.md, AGENTS.md, .cursorrules)"
        ] = "",
    ) -> dict[str, Any]:
        """Return or write the Contextro agent bootstrap block."""
        guard_err = _guard("skill_prompt")
        if guard_err:
            return guard_err

        from contextro_mcp.artifacts.bootstrap import build_bootstrap_block, write_bootstrap

        if target_path:
            try:
                result = write_bootstrap(Path(target_path))
            except ValueError as e:
                return {"error": str(e)}
            return _apply_disclosure(result, tool_name="skill_prompt", preview_keys=["path", "changed"])
        return _apply_disclosure({"content": build_bootstrap_block()}, tool_name="skill_prompt")

    @mcp.tool()
    def docs_bundle(
        output_dir: Annotated[str, "Directory to write the docs bundle into"] = "",
    ) -> dict[str, Any]:
        """Write a packaged local docs bundle."""
        guard_err = _guard("docs_bundle")
        if guard_err:
            return guard_err

        state, err = _require_indexed()
        if err:
            return err

        from contextro_mcp.artifacts.docs_bundle import write_docs_bundle

        try:
            return _apply_disclosure(
                write_docs_bundle(state, output_dir or None),
                tool_name="docs_bundle",
                preview_keys=["output_dir"],
                max_list_items=8,
            )
        except ValueError as e:
            return {"error": str(e)}

    # --- Memory helpers ---

    def _get_memory_store():
        """Lazily initialize the memory store."""
        from contextro_mcp.config import get_settings
        from contextro_mcp.indexing.embedding_service import get_embedding_service
        from contextro_mcp.state import get_state

        state = get_state()
        if state.memory_store is None:
            with _memory_store_lock:
                if state.memory_store is None:
                    settings = get_settings()
                    embedding_svc = get_embedding_service(settings.embedding_model)
                    from contextro_mcp.indexing.embedding_service import EMBEDDING_MODELS
                    from contextro_mcp.memory.memory_store import MemoryStore

                    model_config = EMBEDDING_MODELS.get(settings.embedding_model, {})
                    state.memory_store = MemoryStore(
                        db_path=str(settings.lancedb_path),
                        embedding_service=embedding_svc,
                        vector_dims=model_config.get("dimensions", 768),
                    )
        return state.memory_store

    def _get_compaction_archive():
        """Lazily initialize the durable compaction archive."""
        from contextro_mcp.config import get_settings
        from contextro_mcp.indexing.embedding_service import EMBEDDING_MODELS, get_embedding_service
        from contextro_mcp.memory.compaction_archive import CompactionArchive
        from contextro_mcp.memory.memory_store import MemoryStore
        from contextro_mcp.state import get_state

        state = get_state()
        if not hasattr(state, "_compaction_archive") or state._compaction_archive is None:
            with _compaction_archive_lock:
                if not hasattr(state, "_compaction_archive") or state._compaction_archive is None:
                    settings = get_settings()
                    embedding_svc = get_embedding_service(settings.embedding_model)
                    model_config = EMBEDDING_MODELS.get(settings.embedding_model, {})
                    archive_store = MemoryStore(
                        db_path=str(settings.lancedb_path),
                        embedding_service=embedding_svc,
                        table_name="compaction_archive",
                        vector_dims=model_config.get("dimensions", 256),
                    )
                    state._compaction_archive = CompactionArchive(
                        max_entries=20,
                        ttl=86400.0,
                        storage_path=settings.storage_path / "compaction_archive.json",
                        memory_store=archive_store,
                    )
        return state._compaction_archive

    @mcp.tool()
    def remember(
        content: Annotated[str, "Memory content to store"],
        memory_type: Annotated[str, "Type of memory (e.g. 'note', 'decision')"] = "note",
        tags: Annotated[str, "Comma-separated tags for organization"] = "",
        ttl: Annotated[
            str, "Time-to-live: 'permanent', 'month', 'week', 'day', 'session'"
        ] = "permanent",
        project: Annotated[str, "Project name for scoping memories"] = "default",
    ) -> dict[str, Any]:
        """Store a semantic memory for later recall."""
        import uuid

        from contextro_mcp.core.models import Memory, MemoryType

        guard_err = _guard("remember")
        if guard_err:
            return guard_err

        try:
            mem_type = MemoryType.from_string(memory_type)
        except ValueError:
            return {"error": f"Invalid memory_type: {memory_type}"}

        tag_list = [t.strip() for t in tags.split(",") if t.strip()] if tags else []

        try:
            mem = Memory(
                id=str(uuid.uuid4()),
                content=content,
                memory_type=mem_type,
                project=project,
                tags=tag_list,
                ttl=ttl,
            )
        except ValueError as e:
            return {"error": str(e)}

        store = _get_memory_store()
        mem_id = store.remember(mem)
        return {"id": mem_id, "status": "stored"}

    @mcp.tool(annotations={"readOnlyHint": True})
    def recall(
        query: Annotated[str, "Natural language search query"],
        limit: Annotated[int, "Maximum number of results (default 5)"] = 5,
        memory_type: Annotated[str, "Filter by memory type (e.g. 'note', 'decision')"] = "",
        tags: Annotated[str, "Comma-separated tags to filter by"] = "",
    ) -> dict[str, Any]:
        """Search memories by semantic similarity."""
        guard_err = _guard("recall")
        if guard_err:
            return guard_err

        err = _validate_query(query)
        if err:
            return err

        # Search compaction archive if memory_type is 'archive'
        if memory_type == "archive":
            archive = _get_compaction_archive()
            if archive.size == 0:
                return {
                    "query": query,
                    "total": 0,
                    "results": [],
                    "hint": "No compaction archive exists yet.",
                }
            results = archive.search(query, limit=limit)
            return _apply_disclosure(
                {"query": query, "source": "archive", "total": len(results), "results": results},
                tool_name="recall",
                preview_keys=["query", "source", "total"],
                max_list_items=4,
            )

        store = _get_memory_store()
        tag_list = [t.strip() for t in tags.split(",") if t.strip()] if tags else None

        memories = store.recall(
            query=query,
            limit=max(1, min(limit, 50)),
            memory_type=memory_type,
            tags=tag_list,
        )

        return _apply_disclosure(
            {
                "query": query,
                "total": len(memories),
                "memories": [m.to_dict() for m in memories],
            },
            tool_name="recall",
            preview_keys=["query", "total"],
            max_list_items=4,
        )

    @mcp.tool()
    def forget(
        memory_id: Annotated[str, "Specific memory ID to delete"] = "",
        tags: Annotated[str, "Delete memories matching any of these comma-separated tags"] = "",
        memory_type: Annotated[str, "Delete all memories of this type"] = "",
    ) -> dict[str, Any]:
        """Delete memories by ID, tags, or type."""
        guard_err = _guard("forget")
        if guard_err:
            return guard_err

        store = _get_memory_store()
        tag_list = [t.strip() for t in tags.split(",") if t.strip()] if tags else None

        deleted = store.forget(
            memory_id=memory_id,
            tags=tag_list,
            memory_type=memory_type,
        )

        return {"deleted_count": deleted}

    @mcp.tool()
    def knowledge(
        command: Annotated[
            str,
            "Operation: 'show', 'add', 'remove', 'clear', 'search', 'update', 'status', 'cancel'",
        ],
        name: Annotated[str, "A descriptive name for the knowledge context"] = "",
        value: Annotated[str, "Content to store (text or file/directory path)"] = "",
        query: Annotated[str, "Search query (required for 'search')"] = "",
        context_id: Annotated[str, "Context ID for targeted operations"] = "",
        path: Annotated[str, "File or directory path for 'remove' or 'update'"] = "",
        limit: Annotated[int, "Max search results"] = 5,
        snippet_length: Annotated[int, "Max chars per snippet"] = 500,
        sort_by: Annotated[str, "Sort: 'relevance', 'path', or 'name'"] = "relevance",
        file_type: Annotated[str, "Filter by file type (e.g. 'Code', 'Markdown', 'Text')"] = "",
        offset: Annotated[int, "Pagination offset"] = 0,
        operation_id: Annotated[str, "Operation ID to cancel"] = "",
    ) -> dict[str, Any]:
        """A tool for indexing and searching content across chat sessions using semantic search.

        ## Overview
        Enables persistent storage and retrieval of information using semantic search.
        Content remains available across sessions for later use.

        ## When to use
        - When users ask to query your knowledge bases or kbs
        - When you need to search previously indexed content
        - When users request to index new content (code, markdown, CSV, PDF, and other text file formats)
        - When exploring unfamiliar content to find relevant information

        ## Commands
        - 'show': List all knowledge contexts
        - 'add': Add content to knowledge base (requires 'name' and 'value')
        - 'remove': Remove content (requires 'name', 'context_id', or 'path')
        - 'clear': Remove all knowledge contexts
        - 'search': Search across contexts (requires 'query')
        - 'update': Update existing context (requires 'path' and 'name' or 'context_id')
        - 'status': Show background operation status
        - 'cancel': Cancel background operations
        """
        guard_err = _guard("knowledge")
        if guard_err:
            return guard_err

        import uuid as _uuid

        from contextro_mcp.config import get_settings as _kgs
        from contextro_mcp.state import get_state as _kgs2

        state = _kgs2()
        settings = _kgs()

        # Lazy-init knowledge store (reuses memory store infrastructure)
        if not hasattr(state, "_knowledge_store") or state._knowledge_store is None:
            from contextro_mcp.indexing.embedding_service import (
                EMBEDDING_MODELS,
                get_embedding_service,
            )
            from contextro_mcp.memory.memory_store import MemoryStore

            emb_svc = get_embedding_service(settings.embedding_model)
            model_config = EMBEDDING_MODELS.get(settings.embedding_model, {})
            state._knowledge_store = MemoryStore(
                db_path=str(settings.lancedb_path),
                embedding_service=emb_svc,
                table_name="knowledge",
                vector_dims=model_config.get("dimensions", 256),
            )
        store = state._knowledge_store

        # Knowledge contexts registry (in-memory, keyed by context_id)
        if not hasattr(state, "_knowledge_contexts"):
            state._knowledge_contexts = {}  # context_id -> {name, path, count, created_at}

        import time as _time

        if command == "show":
            contexts = list(state._knowledge_contexts.values())
            return _apply_disclosure(
                {"contexts": contexts, "total": len(contexts)},
                tool_name="knowledge",
                preview_keys=["total"],
                max_list_items=8,
            )

        elif command == "add":
            if not name:
                return {"error": "name required for 'add'"}
            if not value:
                return {"error": "value required for 'add'"}

            ctx_id = str(_uuid.uuid4())[:8]
            chunks_added = 0

            # Check if value is a file/directory path
            val_path = Path(value)
            if val_path.exists():
                # Index file or directory
                from contextro_mcp.indexing.pipeline import SKIP_DIRS

                texts = []
                paths_indexed = []
                if val_path.is_file():
                    try:
                        text = val_path.read_text(errors="replace")
                        texts.append((str(val_path), text))
                        paths_indexed.append(str(val_path))
                    except Exception as e:
                        return {"error": f"Failed to read {value}: {e}"}
                elif val_path.is_dir():
                    for fp in sorted(val_path.rglob("*")):
                        if fp.is_file() and fp.stat().st_size < 1_000_000:
                            if any(skip in fp.parts for skip in SKIP_DIRS):
                                continue
                            try:
                                text = fp.read_text(errors="replace")
                                texts.append((str(fp), text))
                                paths_indexed.append(str(fp))
                            except Exception:
                                continue

                # Chunk and store each file
                for file_path_str, text in texts:
                    # Simple chunking: split into ~500 char chunks with overlap
                    chunk_size = 500
                    overlap = 50
                    for i in range(0, len(text), chunk_size - overlap):
                        chunk = text[i : i + chunk_size]
                        if not chunk.strip():
                            continue
                        from contextro_mcp.core.models import Memory, MemoryType

                        mem = Memory(
                            id=str(_uuid.uuid4()),
                            content=chunk,
                            memory_type=MemoryType.NOTE,
                            project=ctx_id,
                            tags=[name, ctx_id, file_path_str.rsplit("/", 1)[-1]],
                            ttl="permanent",
                            source=file_path_str,
                        )
                        store.remember(mem)
                        chunks_added += 1
            else:
                # Store as plain text
                from contextro_mcp.core.models import Memory, MemoryType

                mem = Memory(
                    id=str(_uuid.uuid4()),
                    content=value,
                    memory_type=MemoryType.NOTE,
                    project=ctx_id,
                    tags=[name, ctx_id],
                    ttl="permanent",
                    source="text",
                )
                store.remember(mem)
                chunks_added = 1

            state._knowledge_contexts[ctx_id] = {
                "context_id": ctx_id,
                "name": name,
                "path": value if Path(value).exists() else "",
                "chunks": chunks_added,
                "created_at": _time.strftime("%Y-%m-%dT%H:%M:%S"),
            }
            return {
                "context_id": ctx_id,
                "name": name,
                "chunks_indexed": chunks_added,
                "status": "indexed",
            }

        elif command == "search":
            if not query:
                return {"error": "query required for 'search'"}

            # Filter by context if specified
            project_filter = context_id if context_id else ""
            tag_filter = [context_id] if context_id else None

            memories = store.recall(
                query=query,
                limit=limit + offset,
                project=project_filter,
                tags=tag_filter,
            )
            memories = memories[offset : offset + limit]

            results = []
            for mem in memories:
                snippet = mem.content[:snippet_length]
                entry: dict[str, Any] = {
                    "content": snippet,
                    "source": mem.source or "",
                    "context_id": mem.project,
                }
                # Find context name
                ctx = state._knowledge_contexts.get(mem.project, {})
                if ctx:
                    entry["name"] = ctx.get("name", "")
                results.append(entry)

            # Sort if requested
            if sort_by == "path":
                results.sort(key=lambda x: x.get("source", ""))
            elif sort_by == "name":
                results.sort(key=lambda x: x.get("name", ""))

            return _apply_disclosure(
                {"query": query, "total": len(results), "results": results},
                tool_name="knowledge",
                preview_keys=["query", "total"],
                max_list_items=6,
            )

        elif command == "remove":
            target_id = context_id
            if not target_id and name:
                # Find by name
                for cid, ctx in state._knowledge_contexts.items():
                    if ctx.get("name") == name:
                        target_id = cid
                        break
            if not target_id and path:
                for cid, ctx in state._knowledge_contexts.items():
                    if ctx.get("path") == path:
                        target_id = cid
                        break
            if not target_id:
                return {"error": "Specify context_id, name, or path to remove"}

            deleted = store.forget(tags=[target_id])
            state._knowledge_contexts.pop(target_id, None)
            return {"removed": target_id, "chunks_deleted": deleted}

        elif command == "clear":
            for ctx_id in list(state._knowledge_contexts.keys()):
                store.forget(tags=[ctx_id])
            count = len(state._knowledge_contexts)
            state._knowledge_contexts.clear()
            return {"cleared": count}

        elif command == "status":
            return {"contexts": len(state._knowledge_contexts), "store_count": store.count()}

        elif command == "update":
            target_id = context_id
            if not target_id and name:
                for cid, ctx in state._knowledge_contexts.items():
                    if ctx.get("name") == name:
                        target_id = cid
                        break
            if not target_id:
                return {"error": "Specify context_id or name to update"}
            # Resolve the path to re-index: use provided path, or fall back to stored path
            existing_ctx = state._knowledge_contexts.get(target_id, {})
            update_path = path or existing_ctx.get("path", "")
            if not update_path:
                return {
                    "error": (
                        "path required for 'update' when the context was created from inline text. "
                        "Provide a new path or use knowledge(command='remove') then knowledge(command='add')."
                    )
                }
            # Remove old, re-add
            store.forget(tags=[target_id])
            state._knowledge_contexts.pop(target_id, None)
            # Re-add with same name, using resolved path
            ctx_name = name or existing_ctx.get("name") or target_id
            return knowledge(command="add", name=ctx_name, value=update_path)

        elif command == "cancel":
            return {"status": "no background operations to cancel"}

        else:
            return {"error": f"Unknown command: {command}"}

    # --- Git Integration Helpers ---

    def _get_commit_indexer():
        """Lazily initialize the commit history indexer."""
        from contextro_mcp.config import get_settings
        from contextro_mcp.indexing.embedding_service import EMBEDDING_MODELS, get_embedding_service
        from contextro_mcp.state import get_state

        state = get_state()
        if state.commit_indexer is None:
            settings = get_settings()
            # Reuse the pipeline's embedding service if already loaded (avoids double model load)
            embedding_svc = (
                state.vector_engine._embedding_service
                if state.vector_engine and hasattr(state.vector_engine, "_embedding_service")
                else get_embedding_service(settings.embedding_model)
            )
            model_config = EMBEDDING_MODELS.get(settings.embedding_model, {})
            from contextro_mcp.git.commit_indexer import CommitHistoryIndexer

            state.commit_indexer = CommitHistoryIndexer(
                embedding_service=embedding_svc,
                vector_dims=model_config.get("dimensions", 256),
            )
        return state.commit_indexer

    def _get_cross_repo_manager():
        """Lazily initialize the cross-repo manager."""
        from contextro_mcp.state import get_state

        state = get_state()
        if state.cross_repo_manager is None:
            from contextro_mcp.git.cross_repo import CrossRepoManager

            state.cross_repo_manager = CrossRepoManager()
        return state.cross_repo_manager

    def _get_branch_watcher():
        """Lazily initialize the branch watcher."""
        from contextro_mcp.state import get_state

        state = get_state()
        if state.branch_watcher is None:
            from contextro_mcp.git.branch_watcher import RealtimeIndexManager

            def _reindex_callback(repo_path: str, full: bool) -> None:
                """Called by branch watcher when reindex is needed."""
                global _pipeline
                if _pipeline is None:
                    return
                try:
                    p = Path(repo_path)
                    if full:
                        result = _pipeline.index(p)
                    else:
                        result = _pipeline.incremental_index(p)
                    # Update state
                    state = get_state()
                    state.vector_engine = _pipeline.vector_engine
                    state.bm25_engine = _pipeline.bm25_engine
                    state.graph_engine = _pipeline.graph_engine
                    settings = _get_settings()
                    _warm_search_pipeline(state, settings)
                    state.capture_index_snapshot()
                    logger.info(
                        "Auto-reindex complete: %d files, %d symbols",
                        result.total_files,
                        result.total_symbols,
                    )
                except Exception as e:
                    logger.error("Auto-reindex failed: %s", e)

            def _commit_reindex_callback(repo_path: str) -> None:
                """Called by branch watcher to reindex commits on branch switch."""
                try:
                    indexer = _get_commit_indexer()
                    settings = _get_settings()
                    indexer.index_commits(
                        repo_path=repo_path,
                        db_path=str(settings.lancedb_path),
                        limit=settings.commit_history_limit,
                        since=settings.commit_history_since or None,
                        include_diffs=settings.commit_include_diffs,
                    )
                except Exception as e:
                    logger.error("Commit reindex failed: %s", e)

            state.branch_watcher = RealtimeIndexManager(
                reindex_callback=_reindex_callback,
                commit_index_callback=_commit_reindex_callback,
                poll_interval=_settings.branch_poll_interval,
                debounce_seconds=_settings.reindex_debounce_seconds,
            )
        return state.branch_watcher

    # --- Commit History Tools ---

    @mcp.tool(annotations={"readOnlyHint": True})
    def commit_history(
        path: Annotated[str, "Repository path (default: indexed codebase)"] = "",
        limit: Annotated[int, "Max commits to return (default 50, max 500)"] = 50,
        since: Annotated[str, "Only commits after this date (e.g. '3 months ago')"] = "",
    ) -> dict[str, Any]:
        """Browse recent commit history for a repository.

        Returns commit metadata including author, message, files changed,
        and change statistics. Use commit_search for semantic queries.
        """
        guard_err = _guard("commit_history")
        if guard_err:
            return guard_err

        from contextro_mcp.git.commit_indexer import (
            extract_commits,
            get_current_branch,
            is_git_repo,
        )
        from contextro_mcp.state import get_state

        state = get_state()

        # Resolve repo path
        if path:
            resolved, err = _validate_path(path)
            if err:
                return err
            repo_path = str(resolved)
        elif state.codebase_path:
            repo_path = str(state.codebase_path)
        else:
            return {"error": "No repository path specified and no codebase indexed."}

        if not is_git_repo(repo_path):
            return {"error": f"Not a git repository: {repo_path}"}

        limit = max(1, min(limit, 500))
        commits = extract_commits(
            repo_path,
            limit=limit,
            since=since or None,
        )

        branch = get_current_branch(repo_path)

        # Compact output: strip diff_summary, repo_path, email; cap files_changed
        compact_commits = []
        for c in commits:
            entry: dict[str, Any] = {
                "hash": c.short_hash,
                "author": c.author_name,
                "date": c.timestamp[:10] if len(c.timestamp) >= 10 else c.timestamp,
                "message": c.message[:200] if len(c.message) > 200 else c.message,
                "files": len(c.files_changed),
            }
            # Only include top 5 file names (not full paths) for context
            if c.files_changed:
                short_files = [f.rsplit("/", 1)[-1] for f in c.files_changed[:5]]
                if len(c.files_changed) > 5:
                    short_files.append(f"...+{len(c.files_changed) - 5} more")
                entry["top_files"] = short_files
            compact_commits.append(entry)

        return _apply_disclosure(
            {
                "total": len(compact_commits),
                "branch": branch,
                "commits": compact_commits,
            },
            tool_name="commit_history",
            preview_keys=["total", "branch"],
            max_list_items=6,
        )

    @mcp.tool(annotations={"readOnlyHint": True})
    def commit_search(
        query: Annotated[str, "Natural language query (e.g. 'auth refactoring')"],
        limit: Annotated[int, "Max results (default 10)"] = 10,
        branch: Annotated[str, "Filter by branch name"] = "",
        author: Annotated[str, "Filter by author name or email"] = "",
    ) -> dict[str, Any]:
        """Semantic search over git commit history.

        Finds commits by meaning, not just keywords. Useful for:
        - "When was the auth module refactored?"
        - "Show commits that changed the payment flow"
        - "What did Alice work on last week?"

        Requires commit history to be indexed (happens automatically during 'index').
        """
        guard_err = _guard("commit_search")
        if guard_err:
            return guard_err

        err = _validate_query(query)
        if err:
            return err

        from contextro_mcp.config import get_settings
        from contextro_mcp.state import get_state

        state = get_state()
        settings = get_settings()

        if not settings.commit_history_enabled:
            return {
                "error": "Commit history indexing is disabled. Set CTX_COMMIT_HISTORY_ENABLED=true."
            }

        indexer = _get_commit_indexer()
        limit = max(1, min(limit, 100))

        repo_path = str(state.codebase_path) if state.codebase_path else None

        results = indexer.search_commits(
            db_path=str(settings.lancedb_path),
            query=query,
            limit=limit,
            branch=branch or None,
            author=author or None,
            repo_path=repo_path,
        )

        return _apply_disclosure(
            {
                "query": query,
                "total": len(results),
                "results": results,
            },
            tool_name="commit_search",
            preview_keys=["query", "total"],
            max_list_items=6,
        )

    # --- Cross-Repo Tools ---

    @mcp.tool()
    def repo_add(
        path: Annotated[str, "Absolute path to the repository to add"],
        name: Annotated[str, "Short name for the repo (default: directory name)"] = "",
        index_now: Annotated[bool, "Index the repo immediately (default True)"] = True,
    ) -> dict[str, Any]:
        """Add a repository to cross-repo context.

        Registers a new repository for unified search across multiple codebases.
        Optionally indexes it immediately. Use repo_status to see all repos.
        """
        guard_err = _guard("repo_add")
        if guard_err:
            return guard_err

        resolved, err = _validate_path(path)
        if err:
            return err

        from contextro_mcp.config import get_settings
        from contextro_mcp.indexing.pipeline import IndexingPipeline
        from contextro_mcp.state import get_state

        global _pipeline

        manager = _get_cross_repo_manager()
        ctx = manager.register_repo(str(resolved), name=name or None)

        result_data = {"status": "registered", "repo": ctx.to_dict()}

        if index_now:
            settings = get_settings()
            with _pipeline_lock:
                if _pipeline is None:
                    _pipeline = IndexingPipeline(settings)

            # Index the repo
            try:
                metadata_path = settings.storage_path / "index_metadata.json"
                if metadata_path.exists():
                    idx_result = _pipeline.incremental_index(resolved)
                else:
                    idx_result = _pipeline.index(resolved)

                # Update state
                state = get_state()
                state.vector_engine = _pipeline.vector_engine
                state.bm25_engine = _pipeline.bm25_engine
                state.graph_engine = _pipeline.graph_engine
                _warm_search_pipeline(state, settings)

                # Update repo stats
                manager.update_repo_stats(
                    str(resolved),
                    total_files=idx_result.total_files,
                    total_symbols=idx_result.total_symbols,
                    total_chunks=idx_result.total_chunks,
                )

                # Index commits if enabled
                if settings.commit_history_enabled:
                    from contextro_mcp.git.commit_indexer import is_git_repo

                    if is_git_repo(str(resolved)):
                        try:
                            indexer = _get_commit_indexer()
                            commit_result = indexer.index_commits(
                                repo_path=str(resolved),
                                db_path=str(settings.lancedb_path),
                                limit=settings.commit_history_limit,
                                since=settings.commit_history_since or None,
                                include_diffs=settings.commit_include_diffs,
                            )
                            manager.update_repo_stats(
                                str(resolved),
                                total_commits=commit_result.get("total_commits", 0),
                            )
                        except Exception as e:
                            logger.warning("Commit indexing failed for %s: %s", resolved, e)

                state.capture_index_snapshot()

                # Start watching if enabled
                if settings.realtime_indexing_enabled:
                    watcher = _get_branch_watcher()
                    watcher.add_repo(str(resolved))
                    if not watcher.is_running:
                        watcher.start()

                result_data["status"] = "registered_and_indexed"
                result_data["index"] = idx_result.to_dict()
                result_data["repo"] = manager.get_repo(str(resolved)).to_dict()

            except Exception as e:
                result_data["index_error"] = str(e)

        return result_data

    @mcp.tool()
    def repo_remove(
        path: Annotated[str, "Path of the repository to remove"] = "",
        name: Annotated[str, "Name of the repository to remove"] = "",
    ) -> dict[str, Any]:
        """Remove a repository from cross-repo context."""
        guard_err = _guard("repo_remove")
        if guard_err:
            return guard_err

        manager = _get_cross_repo_manager()

        if name and not path:
            ctx = manager.get_repo_by_name(name)
            if ctx:
                path = ctx.path
            else:
                return {"error": f"Repository '{name}' not found."}

        if not path:
            return {"error": "Specify either path or name."}

        # Stop watching
        from contextro_mcp.state import get_state

        state = get_state()
        if state.branch_watcher:
            state.branch_watcher.remove_repo(path)

        removed = manager.unregister_repo(path)
        return {"status": "removed" if removed else "not_found", "removed": removed}

    @mcp.tool(annotations={"readOnlyHint": True})
    def repo_status() -> dict[str, Any]:
        """Get status of all registered repositories in cross-repo context.

        Shows indexing state, branch info, and statistics for each repo.
        Also shows branch watcher status.
        """
        guard_err = _guard("repo_status")
        if guard_err:
            return guard_err

        from contextro_mcp.state import get_state

        state = get_state()
        manager = _get_cross_repo_manager()

        result = manager.get_all_status()

        # Add watcher status
        if state.branch_watcher:
            result["watcher"] = state.branch_watcher.get_status()
        else:
            result["watcher"] = {"running": False}

        # Add current branch info
        if state.current_branch:
            result["current_branch"] = state.current_branch
        if state.current_head:
            result["current_head"] = state.current_head[:12]

        return _apply_disclosure(
            result,
            tool_name="repo_status",
            preview_keys=[
                "total_repos",
                "total_files",
                "total_symbols",
                "total_chunks",
                "total_commits",
                "current_branch",
                "current_head",
                "watcher",
            ],
            max_list_items=6,
        )

    # --- Session Context Tool ---

    @mcp.tool(annotations={"readOnlyHint": True})
    def session_snapshot() -> dict[str, Any]:
        """Get a compressed snapshot of the current session for context recovery.

        Use after context compaction to restore awareness of what happened.
        Returns a priority-tiered summary of searches, symbols explored,
        and key actions taken during this session.

        Returns:
            Compact session state (~500 tokens max).
        """
        guard_err = _guard("session_snapshot")
        if guard_err:
            return guard_err

        from contextro_mcp.state import get_state

        state = get_state()

        # Always include codebase context regardless of tracker state
        codebase_ctx: dict[str, Any] = {}
        if state.codebase_path:
            codebase_ctx["codebase"] = str(state.codebase_path)
        if state.current_branch:
            codebase_ctx["branch"] = state.current_branch
        if state.current_head:
            codebase_ctx["head"] = state.current_head[:12]
        if state.vector_engine:
            codebase_ctx["vector_chunks"] = state.vector_engine.count()
        if state.graph_engine:
            stats = state.graph_engine.get_statistics()
            codebase_ctx["symbols"] = stats.get("total_nodes", 0)

        if not hasattr(state, "_session_tracker") or state._session_tracker is None:
            return {**codebase_ctx, "events": 0, "hint": "No session activity tracked yet."}

        snapshot = state._session_tracker.get_snapshot(max_tokens=500)

        # Include archive availability hint
        archive = _get_compaction_archive()
        archive_size = archive.size
        if archive_size > 0:
            snapshot["archive_entries"] = archive_size
            snapshot["archive_hint"] = (
                "Previous session context archived. "
                "Use recall(query, memory_type='archive') to search it."
            )

        return {**codebase_ctx, **snapshot}

    @mcp.tool()
    def compact(
        content: Annotated[
            str, "The pre-compaction context to archive (message history or summary)"
        ],
    ) -> dict[str, Any]:
        """Archive session context before compaction for later searchable recovery.

        Call this before or during context compaction to preserve the full
        session history. The archived content becomes searchable via
        recall(query, memory_type='archive').

        Returns:
            Archive reference and confirmation.
        """
        guard_err = _guard("compact")
        if guard_err:
            return guard_err

        if not content or not content.strip():
            return {"error": "Content cannot be empty."}

        from contextro_mcp.state import get_state

        state = get_state()
        archive = _get_compaction_archive()

        # Include session metadata
        metadata: dict[str, Any] = {}
        if hasattr(state, "_session_tracker") and state._session_tracker is not None:
            metadata["event_count"] = state._session_tracker.event_count

        ref_id = archive.archive(content, metadata=metadata)
        return {
            "archived": True,
            "archive_ref": ref_id,
            "chars": len(content),
            "hint": "Use recall(query, memory_type='archive') to search archived context.",
        }

    @mcp.tool(annotations={"readOnlyHint": True})
    def code(
        operation: Annotated[
            str,
            "Operation: search_symbols, lookup_symbols, get_document_symbols, edit_plan, pattern_search, pattern_rewrite, generate_codebase_overview, search_codebase_map",
        ],
        goal: Annotated[str, "Goal for edit_plan or rewrite planning (optional)"] = "",
        edit_kind: Annotated[str, "Edit category for edit_plan (optional)"] = "",
        symbol_name: Annotated[str, "Symbol name (required for search_symbols)"] = "",
        symbols: Annotated[
            str, "Comma-separated symbol names (required for lookup_symbols, max 10)"
        ] = "",
        file_path: Annotated[
            str,
            "File path (required for get_document_symbols, optional for pattern_search/pattern_rewrite/search_codebase_map)",
        ] = "",
        pattern: Annotated[str, "AST pattern (required for pattern_search/pattern_rewrite)"] = "",
        replacement: Annotated[str, "Replacement pattern (required for pattern_rewrite)"] = "",
        language: Annotated[
            str,
            "Programming language (required for pattern_search/pattern_rewrite, optional for search_symbols)",
        ] = "",
        path: Annotated[
            str,
            "Directory path (optional for search_symbols, generate_codebase_overview, search_codebase_map)",
        ] = "",
        limit: Annotated[int, "Maximum results (optional)"] = 20,
        include_source: Annotated[
            bool, "Include source code in results (optional for lookup_symbols)"
        ] = False,
        top_level_only: Annotated[
            bool, "Only return top-level symbols (optional for get_document_symbols)"
        ] = False,
        dry_run: Annotated[
            bool, "Preview changes without writing (optional for pattern_rewrite)"
        ] = True,
    ) -> dict[str, Any]:
        """Code intelligence with AST parsing and fuzzy search. Language auto-detected from file extension.

        CORE FEATURES:
        • Fuzzy search for symbols (classes, functions, methods)
        • Extracts function/class signatures via AST
        • Structural AST search and rewrite (ast-grep)
        • Codebase overview and directory exploration

        ## Available Operations
        - search_symbols: Find symbol definitions by name
        - lookup_symbols: Batch lookup specific symbols
        - get_document_symbols: List all symbols in a file
        - edit_plan: Plan a scoped edit with targets, risks, and verify steps
        - pattern_search: AST-based structural search
        - pattern_rewrite: AST-based code transformation
        - generate_codebase_overview: High-level codebase structure
        - search_codebase_map: Focused directory exploration
        """
        guard_err = _guard("code")
        if guard_err:
            return guard_err

        from contextro_mcp.indexing.pipeline import SKIP_DIRS
        from contextro_mcp.state import get_state as _gs2

        state = _gs2()

        # --- search_symbols ---
        if operation == "search_symbols":
            if not symbol_name:
                return {"error": "symbol_name required for search_symbols"}
            err = _validate_symbol_name(symbol_name)
            if err:
                return err

            if not state.is_indexed or not state.graph_engine:
                return {"error": "No codebase indexed. Run 'index' first."}

            # Fuzzy search in graph
            matches = state.graph_engine.find_nodes_by_name(symbol_name, exact=False)
            if language:
                matches = [m for m in matches if m.language == language]
            if path:
                matches = [m for m in matches if path in m.location.file_path]

            total = len(matches)
            matches = matches[:limit]
            results = []
            for node in matches:
                entry = _serialize_node(node, state.codebase_path)
                entry.pop("id", None)
                results.append(entry)
            return _apply_disclosure(
                {
                    "operation": "search_symbols",
                    "total": total,
                    "symbols": results,
                },
                tool_name="code",
                preview_keys=["operation", "total"],
                max_list_items=6,
            )

        # --- lookup_symbols ---
        elif operation == "lookup_symbols":
            if not symbols:
                return {"error": "symbols required for lookup_symbols"}
            if not state.is_indexed or not state.graph_engine:
                return {"error": "No codebase indexed. Run 'index' first."}

            names = [s.strip() for s in symbols.split(",") if s.strip()][:10]
            results = {}
            for name in names:
                matches = state.graph_engine.find_nodes_by_name(name, exact=True)
                if not matches:
                    matches = state.graph_engine.find_nodes_by_name(name, exact=False)
                if matches:
                    node = matches[0]
                    entry = _serialize_node(node, state.codebase_path)
                    entry.pop("id", None)
                    if include_source and state.codebase_path:
                        try:
                            fp = Path(node.location.file_path)
                            if not fp.is_absolute() and state.codebase_path:
                                fp = state.codebase_path / fp
                            if fp.exists():
                                lines = fp.read_text(errors="replace").splitlines()
                                start = max(0, node.location.start_line - 1)
                                end = min(
                                    len(lines),
                                    (node.location.end_line or node.location.start_line + 20),
                                )
                                entry["source"] = "\n".join(lines[start:end])
                        except Exception:
                            pass
                    results[name] = entry
                else:
                    results[name] = None
            return _apply_disclosure(
                {
                    "operation": "lookup_symbols",
                    "total": len(results),
                    "symbols": results,
                },
                tool_name="code",
                preview_keys=["operation", "total"],
                max_list_items=6,
            )

        # --- get_document_symbols ---
        elif operation == "get_document_symbols":
            if not file_path:
                return {"error": "file_path required for get_document_symbols"}

            # Resolve path
            fp = Path(file_path)
            if not fp.is_absolute() and state.codebase_path:
                fp = state.codebase_path / fp
            if not fp.exists():
                return {"error": f"File not found: {file_path}"}

            try:
                from contextro_mcp.parsing.treesitter_parser import TreeSitterParser

                parser = TreeSitterParser()
                if not parser.can_parse(str(fp)):
                    return {"error": f"Unsupported file type: {fp.suffix}"}
                parsed = parser.parse_file(str(fp))
                if parsed.error:
                    return {"error": parsed.error}

                syms = parsed.symbols
                if top_level_only:
                    syms = [s for s in syms if not getattr(s, "parent", None)]

                rel_path = file_path
                if state.codebase_path:
                    try:
                        rel_path = str(fp.relative_to(state.codebase_path))
                    except ValueError:
                        pass

                results = []
                for s in syms[:limit]:
                    entry: dict[str, Any] = {
                        "name": s.name,
                        "type": s.type.value if s.type else "unknown",
                        "line": s.line_start,
                    }
                    if s.line_end and s.line_end > s.line_start:
                        entry["end_line"] = s.line_end
                    if s.docstring:
                        entry["doc"] = s.docstring[:100]
                    results.append(entry)

                return _apply_disclosure(
                    {
                        "operation": "get_document_symbols",
                        "file": rel_path,
                        "total": len(parsed.symbols),
                        "symbols": results,
                    },
                    tool_name="code",
                    preview_keys=["operation", "file", "total"],
                    max_list_items=8,
                )
            except Exception as e:
                return {"error": str(e)}

        # --- pattern_search ---
        elif operation == "edit_plan":
            state, err = _require_indexed()
            if err:
                return err

            from contextro_mcp.editing.planner import build_edit_plan

            try:
                result = build_edit_plan(
                    state,
                    goal=goal,
                    edit_kind=edit_kind,
                    symbol_name=symbol_name,
                    file_path=file_path,
                    path=path,
                    pattern=pattern,
                    replacement=replacement,
                    language=language,
                    limit=min(limit, max(1, _settings.edit_plan_max_targets)),
                )
            except ValueError as e:
                return {"error": str(e)}

            _get_tracker().track_edit_plan(
                result.get("goal", goal or symbol_name),
                list(result.get("target_files", [])),
                float(result.get("confidence", 0.0)),
            )
            return _apply_disclosure(
                result,
                tool_name="code",
                preview_keys=[
                    "operation",
                    "goal",
                    "edit_kind",
                    "scope",
                    "primary_target_file",
                    "confidence",
                ],
                max_list_items=6,
            )

        # --- pattern_search ---
        elif operation == "pattern_search":
            if not pattern:
                return {"error": "pattern required for pattern_search"}
            if not language:
                return {"error": "language required for pattern_search"}

            try:
                from ast_grep_py import SgRoot
            except ImportError:
                return {"error": "ast-grep not installed. Install: pip install ast-grep-py"}

            # Determine search scope
            search_root = state.codebase_path
            if path:
                candidate = Path(path)
                if not candidate.is_absolute() and state.codebase_path:
                    candidate = state.codebase_path / path
                if candidate.exists():
                    search_root = candidate

            if not search_root or not search_root.exists():
                return {"error": "No codebase indexed or path not found. Run 'index' first."}

            # Map language to file extensions
            ext_map = {
                "python": [".py"],
                "javascript": [".js", ".jsx", ".mjs"],
                "typescript": [".ts", ".tsx"],
                "rust": [".rs"],
                "go": [".go"],
                "java": [".java"],
                "cpp": [".cpp", ".cc", ".cxx", ".h", ".hpp"],
                "c": [".c", ".h"],
                "ruby": [".rb"],
                "php": [".php"],
                "swift": [".swift"],
                "kotlin": [".kt"],
                "csharp": [".cs"],
            }
            extensions = ext_map.get(language.lower(), [f".{language}"])

            matches = []
            try:
                for ext in extensions:
                    for fp in search_root.rglob(f"*{ext}"):
                        if any(skip in fp.parts for skip in SKIP_DIRS):
                            continue
                        try:
                            source = fp.read_text(errors="replace")
                            root = SgRoot(source, language.lower())
                            node = root.root()
                            found = node.find_all(pattern=pattern)
                            for match in found:
                                rng = match.range()
                                rel = str(fp)
                                if state.codebase_path:
                                    try:
                                        rel = str(fp.relative_to(state.codebase_path))
                                    except ValueError:
                                        pass
                                matches.append(
                                    {
                                        "file": rel,
                                        "line": rng.start.line + 1,
                                        "col": rng.start.column,
                                        "text": match.text()[:200],
                                    }
                                )
                                if len(matches) >= limit:
                                    break
                        except Exception:
                            continue
                        if len(matches) >= limit:
                            break
                    if len(matches) >= limit:
                        break
            except Exception as e:
                return {"error": f"Pattern search failed: {e}"}

            return _apply_disclosure(
                {
                    "operation": "pattern_search",
                    "pattern": pattern,
                    "language": language,
                    "total": len(matches),
                    "matches": matches,
                },
                tool_name="code",
                preview_keys=["operation", "pattern", "language", "total"],
                max_list_items=6,
            )

        # --- pattern_rewrite ---
        elif operation == "pattern_rewrite":
            if not pattern or not replacement:
                return {"error": "pattern and replacement required for pattern_rewrite"}
            if not language:
                return {"error": "language required for pattern_rewrite"}
            if not file_path and not path:
                return {"error": "file_path (single file) or path (directory) required for pattern_rewrite"}

            from contextro_mcp.editing.rewrite import (
                build_rewrite_signature,
                execute_pattern_rewrite,
                has_fresh_preview,
                remember_preview,
            )

            rewrite_signature = build_rewrite_signature(
                pattern=pattern,
                replacement=replacement,
                language=language,
                file_path=file_path,
                path=path,
            )
            if (
                not dry_run
                and _settings.edit_require_preview_before_apply
                and not has_fresh_preview(
                    state,
                    rewrite_signature,
                    ttl_seconds=_settings.edit_preview_ttl_seconds,
                )
            ):
                return {
                    "error": "Preview required before apply. Run pattern_rewrite with dry_run=true first.",
                    "preview_required": True,
                }

            try:
                result = execute_pattern_rewrite(
                    state=state,
                    pattern=pattern,
                    replacement=replacement,
                    language=language,
                    file_path=file_path,
                    path=path,
                    dry_run=dry_run,
                    skip_dirs=SKIP_DIRS,
                    preview_context_lines=_settings.edit_preview_context_lines,
                    preview_max_diff_lines=_settings.edit_preview_max_diff_lines,
                )
            except ImportError:
                return {"error": "ast-grep not installed"}
            except Exception as e:
                return {"error": f"Pattern rewrite failed: {e}"}

            if result.get("error"):
                return result

            touched_files = []
            if result.get("file"):
                touched_files = [result["file"]]
            else:
                touched_files = [entry.get("file", "") for entry in result.get("results", []) if entry.get("file")]
            change_count = int(result.get("changes", result.get("total_changes", 0)) or 0)
            result["preview_signature"] = rewrite_signature
            if dry_run:
                remember_preview(state, rewrite_signature)
                _get_tracker().track_edit_preview(touched_files, change_count)
            else:
                if hasattr(state, "clear_derived_caches"):
                    state.clear_derived_caches()
                _get_tracker().track_edit_apply(touched_files, change_count)
                result["index_hint"] = (
                    "If downstream symbol results look stale, run index() or wait for realtime indexing to refresh."
                )

            return _apply_disclosure(
                result,
                tool_name="code",
                preview_keys=["operation", "file", "changes", "total_changes", "files_modified", "dry_run"],
                max_list_items=10,
            )

        # --- generate_codebase_overview ---
        elif operation == "generate_codebase_overview":
            if not state.is_indexed or not state.graph_engine:
                return {"error": "No codebase indexed. Run 'index' first."}

            graph = state.graph_engine
            stats = graph.get_statistics()

            # Language breakdown
            langs = stats.get("nodes_by_language", {})

            # Top symbols by connectivity
            hub_nodes = []
            for node in list(graph.nodes.values())[:500]:
                in_d, out_d = graph.get_node_degree(node.id)
                if in_d + out_d >= 3:
                    fp = node.location.file_path
                    if state.codebase_path:
                        try:
                            fp = str(Path(fp).relative_to(state.codebase_path))
                        except ValueError:
                            pass
                    hub_nodes.append({"name": node.name, "file": fp, "connections": in_d + out_d})
            hub_nodes.sort(key=lambda x: x["connections"], reverse=True)

            scope = path or str(state.codebase_path or "")
            return _apply_disclosure(
                {
                    "operation": "generate_codebase_overview",
                    "scope": scope,
                    "total_files": stats.get("total_files", 0),
                    "total_symbols": stats.get("total_nodes", 0),
                    "languages": langs,
                    "hub_symbols": hub_nodes[:10],
                    "symbols_by_type": stats.get("nodes_by_type", {}),
                },
                tool_name="code",
                preview_keys=["operation", "scope", "total_files", "total_symbols"],
                max_list_items=8,
            )

        # --- search_codebase_map ---
        elif operation == "search_codebase_map":
            search_root = state.codebase_path
            if path:
                candidate = Path(path)
                if not candidate.is_absolute() and state.codebase_path:
                    candidate = state.codebase_path / path
                if candidate.exists():
                    search_root = candidate

            if not search_root or not search_root.exists():
                return {"error": "No codebase path available. Run 'index' first."}

            # List files and directories with symbol counts
            entries = []
            try:
                for item in sorted(search_root.iterdir()):
                    if item.name.startswith(".") or item.name in SKIP_DIRS:
                        continue
                    if item.is_dir():
                        file_count = sum(1 for _ in item.rglob("*") if _.is_file())
                        entries.append({"name": item.name, "type": "dir", "files": file_count})
                    elif item.is_file():
                        entries.append(
                            {"name": item.name, "type": "file", "size": item.stat().st_size}
                        )
            except Exception as e:
                return {"error": str(e)}

            return _apply_disclosure(
                {
                    "operation": "search_codebase_map",
                    "path": str(search_root),
                    "total": len(entries),
                    "entries": entries[:limit],
                },
                tool_name="code",
                preview_keys=["operation", "path", "total"],
                max_list_items=8,
            )

        else:
            return {
                "error": f"Unknown operation: {operation}. Valid: search_symbols, lookup_symbols, get_document_symbols, edit_plan, pattern_search, pattern_rewrite, generate_codebase_overview, search_codebase_map"
            }

    @mcp.tool(annotations={"readOnlyHint": True})
    def retrieve(
        ref_id: Annotated[str, "Sandbox reference ID (e.g. 'sx_abc12345')"],
        query: Annotated[str, "Optional query to filter content"] = "",
    ) -> dict[str, Any]:
        """Retrieve sandboxed content by reference ID.

        When search results are large, full code snippets are stored in a
        sandbox and only metadata is returned. Use this tool to retrieve
        specific content on demand, reducing context usage.

        Args:
            ref_id: The sandbox reference ID from a previous search result.
            query: Optional filter to return only matching lines.

        Returns:
            The stored content or an error if not found.
        """
        guard_err = _guard("retrieve")
        if guard_err:
            return guard_err

        from contextro_mcp.state import get_state

        state = get_state()

        if not hasattr(state, "_output_sandbox") or state._output_sandbox is None:
            return {"error": "No sandbox content available."}

        content = state._output_sandbox.retrieve(ref_id, query=query or None)
        if content is None:
            return {"error": f"Reference '{ref_id}' not found or expired."}

        return {"ref_id": ref_id, "content": content}

    @mcp.tool(annotations={"readOnlyHint": True})
    def introspect(
        query: Annotated[
            str, "The user's question about this assistant's usage, features, or capabilities"
        ] = "",
        doc_path: Annotated[
            str, "Path to a specific doc to retrieve (e.g. 'features/tangent-mode.md')"
        ] = "",
    ) -> dict[str, Any]:
        """Look up documentation about Contextro's own features, tools, and capabilities.

        - Agent asks about Contextro's features, tools, or settings
        - Agent wants to learn what Contextro can do
        - Agent asks how to use a specific Contextro tool
        - Agent wants configuration or model advice
        - Questions unrelated to Contextro itself
        """
        guard_err = _guard("introspect")
        if guard_err:
            return guard_err

        # Tool reference
        tools_doc = {
            "search": "Semantic + keyword + graph hybrid search. PREFERRED over grep/glob. Returns code chunks with name, file, line, score, code snippet.",
            "code": "AST-based code intelligence. Operations: search_symbols, lookup_symbols, get_document_symbols, edit_plan, pattern_search, pattern_rewrite, generate_codebase_overview, search_codebase_map.",
            "find_symbol": "Find symbol definitions by exact or fuzzy name. Returns location, callers_count, callees_count.",
            "find_callers": "Who calls this function? Returns compact 'name (file:line)' list.",
            "find_callees": "What does this function call? Returns compact 'name (file:line)' list.",
            "explain": "Full explanation of a symbol — definition + callers + callees + related code. PREFERRED over reading files.",
            "impact": "What breaks if I change this? Transitive caller analysis. MUST use before refactoring.",
            "analyze": "Complexity, code smells, quality metrics. Use analyze(path='src/auth') for scoped results.",
            "focus": "One-file, low-token context slice: symbols, imports, blast radius, nearby tests, compressed preview.",
            "restore": "One-shot repo restore/context-bomb summary with entry points, hubs, risks, and session hints.",
            "audit": "Packaged audit report combining quality, dead code, cycles, hubs, and static coverage gaps.",
            "dead_code": "Entry-point reachability report for unused files, private symbols, and unresolved local imports.",
            "circular_dependencies": "SCC-based circular dependency report at file level.",
            "test_coverage_map": "Static test reachability map; reports uncovered production files and symbols.",
            "sidecar_export": "Generate or clean file-adjacent `.graph.*` sidecars for non-MCP agents.",
            "skill_prompt": "Print or update a Contextro bootstrap block in CLAUDE.md, AGENTS.md, or .cursorrules.",
            "docs_bundle": "Write a local docs bundle (index.md, architecture.md, audit.md, llms.txt).",
            "overview": "Project structure at a glance. PREFERRED over ls/glob.",
            "architecture": "Layers, entry points, dependency hubs.",
            "index": "Index a codebase. Runs in background. Poll status() until indexed: true.",
            "status": "Server status, index stats, branch, memory, cache hit rate.",
            "health": "Readiness check.",
            "commit_history": "Browse recent commits with files changed.",
            "commit_search": "Semantic search over git commit history.",
            "remember": "Store a note/decision with tags and TTL.",
            "recall": "Search memories by meaning.",
            "forget": "Delete memories by ID, tag, or type.",
            "knowledge": "Index and search user-provided content (docs, notes, files). Commands: show, add, search, remove, clear, update, status.",
            "session_snapshot": "Compressed session state for context recovery after compaction. Includes codebase path, branch, recent searches, symbols explored.",
            "retrieve": "Fetch sandboxed large output by reference ID.",
            "repo_add": "Register another repository for unified search.",
            "repo_remove": "Unregister a repository.",
            "repo_status": "View all repos, branches, and watcher status.",
        }

        settings_doc = {
            "CTX_EMBEDDING_MODEL": "Embedding model. Default: potion-code-16m (fast, code-specific). Options: potion-8m, bge-small-en, jina-code, nomic-embed.",
            "CTX_STORAGE_DIR": "Where indexes are stored. Default: ~/.contextro",
            "CTX_AUTO_WARM_START": "Restore index from disk on restart. Default: false. Set true in Docker.",
            "CTX_OUTPUT_FORMAT": "json (default) or toon (10% more token savings).",
            "CTX_MAX_MEMORY_MB": "RAM budget. Default: 350.",
            "CTX_SEARCH_MODE": "hybrid (default), vector, or bm25.",
            "CTX_COMMIT_HISTORY_ENABLED": "Index git commits. Default: true.",
            "CTX_COMMIT_INCLUDE_DIFFS": "Include diff summaries in commit embeddings. Default: false for faster/lower-memory indexing; set true for richer commit_search over code changes.",
            "CTX_REALTIME_INDEXING_ENABLED": "Auto-reindex on branch switch. Default: true.",
            "CTX_CODEBASE_HOST_PATH": "Optional Docker helper. Host path the client will send to index().",
            "CTX_CODEBASE_MOUNT_PATH": "Optional Docker helper. In-container mount path for CTX_CODEBASE_HOST_PATH. Default: /repos/platform",
            "CTX_PATH_PREFIX_MAP": "Map host paths to container paths. Format: /host/path=/container/path",
        }

        workflow_doc = {
            "first_use": "1. index('/path/to/project') → 2. poll status() until indexed:true → 3. search/find_symbol/etc.",
            "refactoring": "1. impact('symbol') → 2. code(operation='edit_plan', ...) → 3. code(operation='pattern_rewrite', dry_run=True, ...) → 4. apply and verify",
            "bug_investigation": "1. search('error message') → 2. find_symbol('ErrorClass') → 3. find_callers → 4. commit_search('recent changes')",
            "context_recovery": "After compaction: session_snapshot() → restore awareness of what was done",
            "productized_context": "restore() for repo-wide context → focus(path='...') for one-file context → audit() for risks → sidecar_export() for non-MCP sidecars",
            "token_efficiency": "search < find_symbol < explain < readFile. Use in that order. Only readFile when you need the full body.",
        }

        if doc_path:
            # Return specific section
            if "tool" in doc_path.lower():
                return _apply_disclosure(
                    {"tools": tools_doc},
                    tool_name="introspect",
                    preview_keys=["tools"],
                )
            elif "setting" in doc_path.lower() or "config" in doc_path.lower():
                return _apply_disclosure(
                    {"settings": settings_doc},
                    tool_name="introspect",
                    preview_keys=["settings"],
                )
            elif "workflow" in doc_path.lower():
                return _apply_disclosure(
                    {"workflows": workflow_doc},
                    tool_name="introspect",
                    preview_keys=["workflows"],
                )

        if query:
            q = query.lower()
            result: dict[str, Any] = {}

            # Match query to relevant sections
            tool_keywords = [
                "tool",
                "search",
                "find",
                "explain",
                "impact",
                "index",
                "commit",
                "memory",
                "knowledge",
                "code",
                "introspect",
            ]
            setting_keywords = ["setting", "config", "env", "variable", "ctx_", "model", "storage"]
            workflow_keywords = [
                "workflow",
                "how to",
                "when to",
                "first",
                "refactor",
                "bug",
                "token",
            ]

            if any(k in q for k in tool_keywords):
                # Find matching tools
                matching = {
                    k: v
                    for k, v in tools_doc.items()
                    if k in q or any(word in v.lower() for word in q.split()[:3])
                }
                result["tools"] = matching if matching else tools_doc
            if any(k in q for k in setting_keywords):
                result["settings"] = settings_doc
            if any(k in q for k in workflow_keywords):
                result["workflows"] = workflow_doc

            if not result:
                # Return everything
                result = {"tools": tools_doc, "settings": settings_doc, "workflows": workflow_doc}

            return _apply_disclosure(
                result,
                tool_name="introspect",
                preview_keys=["tools", "settings", "workflows"],
            )

        # Default: return tool list
        return _apply_disclosure(
            {
                "tools": list(tools_doc.keys()),
                "hint": "Use query='search tool' or doc_path='tools' for details",
                "total_tools": len(tools_doc),
            },
            tool_name="introspect",
            preview_keys=["total_tools", "hint"],
            max_list_items=8,
        )

    return mcp


def main(argv: Optional[list[str]] = None):
    """Entry point for contextro CLI."""
    from contextro_mcp import __version__
    from contextro_mcp.config import get_settings
    from contextro_mcp.state import get_state

    def _print_report(data: dict[str, Any], output_format: str) -> None:
        from contextro_mcp.reports.renderers import render_report

        print(render_report(data, output_format))

    def _run_cli_command(args) -> None:
        import asyncio

        from contextro_mcp.artifacts.bootstrap import build_bootstrap_block, write_bootstrap
        from contextro_mcp.artifacts.docs_bundle import write_docs_bundle
        from contextro_mcp.artifacts.graph_workflow import (
            initialize_graph_workflow,
            watch_graph_workflow,
        )
        from contextro_mcp.artifacts.sidecars import clean_sidecars, export_sidecars
        from contextro_mcp.cli.runtime import ensure_indexed_state
        from contextro_mcp.reports.product import (
            build_audit_report,
            build_focus_report,
            build_restore_report,
            get_static_analysis_bundle,
        )

        if args.command in {"skill", "bootstrap"}:
            if getattr(args, "target", ""):
                _print_report(write_bootstrap(Path(args.target)), "human")
            else:
                print(build_bootstrap_block())
            return

        if args.command == "graph":
            if args.graph_command == "watch":
                asyncio.run(
                    watch_graph_workflow(
                        codebase_path=args.codebase,
                        target_path=args.path,
                        include_code=args.include_code,
                        docs_output_dir=args.output_dir,
                        bootstrap_target=args.bootstrap_target or None,
                        include_docs=not args.no_docs,
                        debounce_seconds=args.debounce_seconds,
                        output_format=args.format,
                    )
                )
                return

            state = ensure_indexed_state(getattr(args, "codebase", None))
            _print_report(
                initialize_graph_workflow(
                    state,
                    target_path=args.path,
                    include_code=args.include_code,
                    docs_output_dir=args.output_dir,
                    bootstrap_target=args.bootstrap_target or None,
                    include_docs=not args.no_docs,
                ),
                args.format,
            )
            return

        state = ensure_indexed_state(getattr(args, "codebase", None))

        if args.command == "focus":
            _print_report(
                build_focus_report(state, args.path, include_code=not args.no_code),
                args.format,
            )
            return
        if args.command == "restore":
            _print_report(build_restore_report(state), args.format)
            return
        if args.command == "audit":
            _print_report(build_audit_report(state), args.format)
            return
        if args.command == "dead-code":
            _print_report(get_static_analysis_bundle(state)["dead_code"], args.format)
            return
        if args.command == "circular-deps":
            _print_report(get_static_analysis_bundle(state)["circular_dependencies"], args.format)
            return
        if args.command == "test-coverage":
            _print_report(get_static_analysis_bundle(state)["test_coverage_map"], args.format)
            return
        if args.command == "docs":
            _print_report(write_docs_bundle(state, args.output_dir), args.format)
            return
        if args.command in {"sidecar", "sidecars"}:
            if args.sidecar_command == "clean":
                result = clean_sidecars(state, target_path=args.path)
            else:
                result = export_sidecars(
                    state,
                    target_path=args.path,
                    include_code=args.include_code,
                )
            _print_report(result, args.format)
            return

    parser = argparse.ArgumentParser(
        prog="contextro",
        description="Run the Contextro MCP server locally over stdio or HTTP.",
    )
    parser.add_argument(
        "--transport",
        choices=("stdio", "http"),
        help="Override CTX_TRANSPORT for this process.",
    )
    parser.add_argument(
        "--host",
        help="HTTP host override when using --transport http.",
    )
    parser.add_argument(
        "--port",
        type=int,
        help="HTTP port override when using --transport http.",
    )
    parser.add_argument(
        "--version",
        action="store_true",
        help="Print the Contextro version and exit.",
    )

    subparsers = parser.add_subparsers(dest="command")
    output_choices = ("human", "json", "markdown", "compact")

    server_parser = subparsers.add_parser("server", help="Run the MCP server explicitly.")
    server_parser.add_argument(
        "--transport",
        choices=("stdio", "http"),
        help="Override CTX_TRANSPORT for this process.",
    )
    server_parser.add_argument("--host", help="HTTP host override when using --transport http.")
    server_parser.add_argument("--port", type=int, help="HTTP port override when using --transport http.")

    focus_parser = subparsers.add_parser("focus", help="Build a low-token one-file context slice.")
    focus_parser.add_argument("path", help="File to focus.")
    focus_parser.add_argument("--codebase", help="Optional codebase root to index first.")
    focus_parser.add_argument("--no-code", action="store_true", help="Skip the code preview section.")
    focus_parser.add_argument("--format", choices=output_choices, default="human")

    restore_parser = subparsers.add_parser("restore", help="Build a repo restore/context summary.")
    restore_parser.add_argument("--codebase", help="Optional codebase root to index first.")
    restore_parser.add_argument("--format", choices=output_choices, default="human")

    audit_parser = subparsers.add_parser("audit", help="Run the packaged audit report.")
    audit_parser.add_argument("--codebase", help="Optional codebase root to index first.")
    audit_parser.add_argument("--format", choices=output_choices, default="human")

    dead_code_parser = subparsers.add_parser(
        "dead-code", help="Run entry-point reachability based dead-code analysis."
    )
    dead_code_parser.add_argument("--codebase", help="Optional codebase root to index first.")
    dead_code_parser.add_argument("--format", choices=output_choices, default="human")

    cycles_parser = subparsers.add_parser(
        "circular-deps", help="Run SCC-based circular dependency analysis."
    )
    cycles_parser.add_argument("--codebase", help="Optional codebase root to index first.")
    cycles_parser.add_argument("--format", choices=output_choices, default="human")

    coverage_parser = subparsers.add_parser(
        "test-coverage", help="Run the static test coverage map."
    )
    coverage_parser.add_argument("--codebase", help="Optional codebase root to index first.")
    coverage_parser.add_argument("--format", choices=output_choices, default="human")

    docs_parser = subparsers.add_parser("docs", help="Write the packaged docs bundle.")
    docs_parser.add_argument("--codebase", help="Optional codebase root to index first.")
    docs_parser.add_argument(
        "--output-dir",
        default="",
        help="Directory to write the docs bundle into.",
    )
    docs_parser.add_argument("--format", choices=output_choices, default="human")

    skill_parser = subparsers.add_parser(
        "skill",
        aliases=["bootstrap"],
        help="Print or update the Contextro agent bootstrap block.",
    )
    skill_parser.add_argument("--target", help="Optional file to update in place.")

    sidecar_parser = subparsers.add_parser(
        "sidecar",
        aliases=["sidecars"],
        help="Generate or clean file-adjacent `.graph.*` sidecars.",
    )
    sidecar_parser.add_argument("--codebase", help="Optional codebase root to index first.")
    sidecar_subparsers = sidecar_parser.add_subparsers(dest="sidecar_command", required=True)
    sidecar_export_parser = sidecar_subparsers.add_parser("export", help="Export `.graph.*` sidecars.")
    sidecar_export_parser.add_argument(
        "path",
        nargs="?",
        default=".",
        help="Optional file or directory to export sidecars for.",
    )
    sidecar_export_parser.add_argument(
        "--include-code",
        action="store_true",
        help="Include compressed code previews in the generated sidecars.",
    )
    sidecar_export_parser.add_argument("--format", choices=output_choices, default="human")
    sidecar_clean_parser = sidecar_subparsers.add_parser("clean", help="Remove generated sidecars.")
    sidecar_clean_parser.add_argument(
        "path",
        nargs="?",
        default=".",
        help="Optional file or directory to clean sidecars for.",
    )
    sidecar_clean_parser.add_argument("--format", choices=output_choices, default="human")

    graph_parser = subparsers.add_parser(
        "graph",
        help="Initialize or watch the first-class graph sidecar and docs workflow.",
    )
    graph_parser.add_argument("--codebase", help="Optional codebase root to index first.")
    graph_subparsers = graph_parser.add_subparsers(dest="graph_command", required=True)

    graph_init_parser = graph_subparsers.add_parser(
        "init",
        help="Generate graph sidecars, docs, and optional bootstrap instructions.",
    )
    graph_init_parser.add_argument(
        "path",
        nargs="?",
        default=".",
        help="Optional file or directory to initialize graph artifacts for.",
    )
    graph_init_parser.add_argument(
        "--include-code",
        action="store_true",
        help="Include compressed code previews in generated sidecars.",
    )
    graph_init_parser.add_argument(
        "--output-dir",
        default="",
        help="Directory to write the docs bundle into.",
    )
    graph_init_parser.add_argument(
        "--bootstrap-target",
        default="",
        help="Optional bootstrap file target (CLAUDE.md, AGENTS.md, .cursorrules, or alias).",
    )
    graph_init_parser.add_argument(
        "--no-docs",
        action="store_true",
        help="Skip docs bundle generation.",
    )
    graph_init_parser.add_argument("--format", choices=output_choices, default="human")

    graph_watch_parser = graph_subparsers.add_parser(
        "watch",
        help="Keep sidecars and docs refreshed while editing.",
    )
    graph_watch_parser.add_argument(
        "path",
        nargs="?",
        default=".",
        help="Optional file or directory scope for sidecar refreshes.",
    )
    graph_watch_parser.add_argument(
        "--include-code",
        action="store_true",
        help="Include compressed code previews in generated sidecars.",
    )
    graph_watch_parser.add_argument(
        "--output-dir",
        default="",
        help="Directory to write the docs bundle into.",
    )
    graph_watch_parser.add_argument(
        "--bootstrap-target",
        default="",
        help="Optional bootstrap file target to create during watcher startup.",
    )
    graph_watch_parser.add_argument(
        "--no-docs",
        action="store_true",
        help="Skip docs bundle generation during watcher refreshes.",
    )
    graph_watch_parser.add_argument(
        "--debounce-seconds",
        type=float,
        default=2.0,
        help="Debounce delay before refresh after file changes.",
    )
    graph_watch_parser.add_argument("--format", choices=output_choices, default="human")

    args = parser.parse_args(argv)

    if args.version:
        print(__version__)
        return

    settings = get_settings()
    log_level = getattr(logging, settings.log_level.upper(), logging.INFO)

    if settings.log_format == "json":
        handler = logging.StreamHandler()
        handler.setFormatter(JsonFormatter())
        logging.root.addHandler(handler)
        logging.root.setLevel(log_level)
    else:
        logging.basicConfig(level=log_level)

    def _shutdown_handler(signum, frame):
        logger.info("Received signal %s, shutting down...", signum)
        raise SystemExit(0)

    signal.signal(signal.SIGTERM, _shutdown_handler)
    signal.signal(signal.SIGINT, _shutdown_handler)

    try:
        if args.command not in (None, "server"):
            _run_cli_command(args)
        else:
            server = create_server()
            transport_arg = args.transport if args.command is None else getattr(args, "transport", None)
            host_arg = args.host if args.command is None else getattr(args, "host", None)
            port_arg = args.port if args.command is None else getattr(args, "port", None)
            transport = (transport_arg or os.environ.get("CTX_TRANSPORT", "stdio")).lower()

            if transport == "http":
                host = host_arg or os.environ.get("CTX_HTTP_HOST", "0.0.0.0")
                port = port_arg or int(os.environ.get("CTX_HTTP_PORT", "8000"))
                # HTTP servers are long-lived and shared; auto-warm-start by default
                # so the server is immediately useful without requiring index() on every restart.
                if not settings.auto_warm_start and "CTX_AUTO_WARM_START" not in os.environ:
                    settings.auto_warm_start = True
                    logger.info("HTTP transport: auto_warm_start enabled by default")
                logger.info("Starting Contextro HTTP server on %s:%d", host, port)
                server.run(transport="streamable-http", host=host, port=port, path="/mcp")
            else:
                server.run()
    except ValueError as exc:
        parser.exit(2, f"error: {exc}\n")
    finally:
        get_state().shutdown()


if __name__ == "__main__":
    main()
