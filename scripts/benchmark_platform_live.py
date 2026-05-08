"""Live end-to-end benchmark for Contextro on the platform repository.

This script uses the real server, real embedding path, and a fresh storage
directory. It does not patch embeddings or mock tool behavior.
"""

# ruff: noqa: E402, I001

from __future__ import annotations

import argparse
import asyncio
import gc
import json
import os
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src"))

from benchmark_utils import call_tool, estimate_tokens


SEARCH_CASES = [
    "agent workflow run preparation",
    "prepare issue worktree",
    "start agent runtime convex",
    "partner api mcp configuration",
    "seo pipeline summary",
    "trailflow partner loads server",
]

SYMBOL_CASES = [
    "prepareIssueWorktree",
    "ensureTmuxSession",
    "getPartnerApiMcpConfig",
    "runJsonCommand",
    "buildSummary",
]

CALLER_CASES = [
    "prepareIssueWorktree",
    "getPartnerApiMcpConfig",
    "runJsonCommand",
]


def _summarize_series(values: list[float]) -> dict[str, float]:
    if not values:
        return {"count": 0, "avg_ms": 0.0, "p50_ms": 0.0, "p95_ms": 0.0}

    sorted_values = sorted(values)
    count = len(sorted_values)
    p50 = sorted_values[count // 2]
    p95_index = min(count - 1, max(0, int(count * 0.95) - 1))
    p95 = sorted_values[p95_index]
    return {
        "count": count,
        "avg_ms": round(sum(sorted_values) / count, 2),
        "p50_ms": round(p50, 2),
        "p95_ms": round(p95, 2),
    }


def _summarize_tokens(values: list[int]) -> dict[str, float]:
    if not values:
        return {"count": 0, "avg_tokens": 0.0, "max_tokens": 0}
    return {
        "count": len(values),
        "avg_tokens": round(sum(values) / len(values), 1),
        "max_tokens": max(values),
    }


async def _timed_call(mcp, tool_name: str, args: dict | None = None) -> tuple[dict, float, int]:
    started = time.perf_counter()
    result = await call_tool(mcp, tool_name, args or {})
    elapsed_ms = (time.perf_counter() - started) * 1000
    return result, elapsed_ms, estimate_tokens(result)


async def _index_codebase(mcp, server_module, path: str, timeout_seconds: int) -> dict:
    result = await call_tool(mcp, "index", {"path": path})
    if result.get("status") != "indexing":
        return result

    for _ in range(timeout_seconds * 2):
        await asyncio.sleep(0.5)
        with server_module._index_job_lock:
            job_status = server_module._index_job.get("status")
            job_result = server_module._index_job.get("result", {})
            job_error = server_module._index_job.get("error")

        if job_status == "done" and job_result:
            return job_result
        if job_status == "error" or job_error:
            raise RuntimeError(job_error or "Background indexing failed")

    status = await call_tool(mcp, "status")
    raise TimeoutError(
        f"Indexing did not complete within {timeout_seconds}s (last status: {status})"
    )


def _configure_environment(storage_dir: Path) -> None:
    os.environ["CTX_STORAGE_DIR"] = str(storage_dir)
    os.environ.setdefault("CTX_PERMISSION_LEVEL", "full")


def _ensure_fresh_storage_dir(storage_dir: Path) -> None:
    storage_dir.mkdir(parents=True, exist_ok=True)
    if any(storage_dir.iterdir()):
        raise ValueError(
            f"Storage directory must be empty for a live benchmark: {storage_dir}. "
            "Use a new path for each run."
        )


def _reset_runtime() -> tuple[object, object]:
    import contextro_mcp.server as server_module
    from contextro_mcp.config import reset_settings
    from contextro_mcp.state import reset_state

    reset_settings()
    reset_state()
    server_module._pipeline = None
    server_module._index_job = {}
    return server_module.create_server(), server_module


async def run_benchmark(codebase_path: Path, storage_dir: Path, index_timeout: int) -> dict:
    _configure_environment(storage_dir)
    mcp, server_module = _reset_runtime()

    from contextro_mcp.config import get_settings
    from contextro_mcp.state import get_state

    settings = get_settings()
    initial_embedding_backend = settings.embedding_model

    metrics: dict[str, object] = {
        "timestamp": time.time(),
        "codebase": str(codebase_path),
        "storage_dir": str(storage_dir),
        "environment": {
            "embedding_model": initial_embedding_backend,
            "python": sys.version.split()[0],
        },
        "index": {},
        "status": {},
        "status_after": {},
        "latency_ms": {},
        "tokens": {},
        "search": {},
        "cache": {},
        "memory": {},
        "knowledge": {},
        "tools": {},
        "totals": {},
    }

    try:
        index_started = time.perf_counter()
        index_result = await _index_codebase(
            mcp,
            server_module,
            str(codebase_path),
            timeout_seconds=index_timeout,
        )
        index_elapsed_ms = (time.perf_counter() - index_started) * 1000
        metrics["index"] = {
            "latency_ms": round(index_elapsed_ms, 2),
            "result": index_result,
        }

        state = get_state()
        vector_engine = state.vector_engine
        if (
            vector_engine is not None
            and getattr(vector_engine, "_embedding_service", None) is not None
        ):
            svc = vector_engine._embedding_service
            metrics["environment"]["embedding_backend"] = (
                "model2vec" if getattr(svc, "_is_model2vec", False) else "sentence-transformers"
            )
            metrics["environment"]["embedding_device"] = getattr(svc, "device", "unknown")
            metrics["environment"]["embedding_model_loaded"] = getattr(svc, "model_name", "unknown")
            metrics["environment"]["embedding_cache_hits"] = svc.stats.get("cache_hits", 0)
            metrics["environment"]["embedding_cache_misses"] = svc.stats.get("cache_misses", 0)

        status_result, status_ms, status_tokens = await _timed_call(mcp, "status")
        metrics["status"] = status_result
        metrics["latency_ms"]["status"] = {"avg_ms": round(status_ms, 2)}
        metrics["tokens"]["status"] = {"avg_tokens": status_tokens}

        search_latencies: list[float] = []
        search_tokens: list[int] = []
        search_totals: list[int] = []
        search_samples: list[dict[str, object]] = []
        sandbox_refs: list[str] = []

        for query in SEARCH_CASES:
            result, latency_ms, tokens = await _timed_call(
                mcp, "search", {"query": query, "limit": 10}
            )
            search_latencies.append(latency_ms)
            search_tokens.append(tokens)
            search_totals.append(int(result.get("total", 0)))
            if result.get("sandbox_ref"):
                sandbox_refs.append(str(result["sandbox_ref"]))
            search_samples.append(
                {
                    "query": query,
                    "latency_ms": round(latency_ms, 2),
                    "tokens": tokens,
                    "total": result.get("total", 0),
                    "confidence": result.get("confidence"),
                    "sandboxed": bool(result.get("sandboxed")),
                }
            )

        repeat_latencies: list[float] = []
        repeat_tokens: list[int] = []
        for query in SEARCH_CASES[:3]:
            _result, latency_ms, tokens = await _timed_call(
                mcp, "search", {"query": query, "limit": 10}
            )
            repeat_latencies.append(latency_ms)
            repeat_tokens.append(tokens)

        metrics["latency_ms"]["search"] = _summarize_series(search_latencies)
        metrics["tokens"]["search"] = _summarize_tokens(search_tokens)
        metrics["search"] = {
            "queries": search_samples,
            "avg_results": (
                round(sum(search_totals) / len(search_totals), 2) if search_totals else 0.0
            ),
        }
        metrics["cache"] = {
            "repeat_search_latency_ms": _summarize_series(repeat_latencies),
            "repeat_search_tokens": _summarize_tokens(repeat_tokens),
        }

        find_symbol_latencies: list[float] = []
        find_symbol_tokens: list[int] = []
        for name in SYMBOL_CASES:
            _result, latency_ms, tokens = await _timed_call(mcp, "find_symbol", {"name": name})
            find_symbol_latencies.append(latency_ms)
            find_symbol_tokens.append(tokens)
        metrics["latency_ms"]["find_symbol"] = _summarize_series(find_symbol_latencies)
        metrics["tokens"]["find_symbol"] = _summarize_tokens(find_symbol_tokens)

        find_callers_latencies: list[float] = []
        find_callers_tokens: list[int] = []
        for name in CALLER_CASES:
            _result, latency_ms, tokens = await _timed_call(
                mcp, "find_callers", {"symbol_name": name}
            )
            find_callers_latencies.append(latency_ms)
            find_callers_tokens.append(tokens)
        metrics["latency_ms"]["find_callers"] = _summarize_series(find_callers_latencies)
        metrics["tokens"]["find_callers"] = _summarize_tokens(find_callers_tokens)

        explain_result, explain_ms, explain_tokens = await _timed_call(
            mcp, "explain", {"symbol_name": "prepareIssueWorktree"}
        )
        impact_result, impact_ms, impact_tokens = await _timed_call(
            mcp, "impact", {"symbol_name": "prepareIssueWorktree"}
        )
        overview_result, overview_ms, overview_tokens = await _timed_call(mcp, "overview")
        architecture_result, architecture_ms, architecture_tokens = await _timed_call(
            mcp, "architecture"
        )
        session_result, session_ms, session_tokens = await _timed_call(mcp, "session_snapshot")
        introspect_result, introspect_ms, introspect_tokens = await _timed_call(
            mcp,
            "introspect",
            {"query": "what tools help with search and session recovery"},
        )

        metrics["latency_ms"]["explain"] = {"avg_ms": round(explain_ms, 2)}
        metrics["latency_ms"]["impact"] = {"avg_ms": round(impact_ms, 2)}
        metrics["latency_ms"]["overview"] = {"avg_ms": round(overview_ms, 2)}
        metrics["latency_ms"]["architecture"] = {"avg_ms": round(architecture_ms, 2)}
        metrics["latency_ms"]["session_snapshot"] = {"avg_ms": round(session_ms, 2)}
        metrics["latency_ms"]["introspect"] = {"avg_ms": round(introspect_ms, 2)}

        metrics["tokens"]["explain"] = {"avg_tokens": explain_tokens}
        metrics["tokens"]["impact"] = {"avg_tokens": impact_tokens}
        metrics["tokens"]["overview"] = {"avg_tokens": overview_tokens}
        metrics["tokens"]["architecture"] = {"avg_tokens": architecture_tokens}
        metrics["tokens"]["session_snapshot"] = {"avg_tokens": session_tokens}
        metrics["tokens"]["introspect"] = {"avg_tokens": introspect_tokens}

        metrics["tools"]["overview"] = {
            "total_files": overview_result.get("total_files", 0),
            "total_symbols": overview_result.get("total_symbols", 0),
            "vector_chunks": overview_result.get("vector_chunks", 0),
        }
        metrics["tools"]["architecture"] = {
            "layers": len(architecture_result.get("layers", {})),
            "entry_points": len(architecture_result.get("entry_points", [])),
            "hub_symbols": len(architecture_result.get("hub_symbols", [])),
        }
        metrics["tools"]["explain"] = {"keys": sorted(explain_result.keys())}
        metrics["tools"]["impact"] = {"total_impacted": impact_result.get("total_impacted", 0)}
        metrics["tools"]["session_snapshot"] = {"keys": sorted(session_result.keys())}
        metrics["tools"]["introspect"] = {"keys": sorted(introspect_result.keys())}

        code_overview_result, code_overview_ms, code_overview_tokens = await _timed_call(
            mcp,
            "code",
            {"operation": "generate_codebase_overview"},
        )
        code_map_result, code_map_ms, code_map_tokens = await _timed_call(
            mcp,
            "code",
            {"operation": "search_codebase_map", "path": "scripts", "limit": 20},
        )
        document_symbols_result, document_symbols_ms, document_symbols_tokens = await _timed_call(
            mcp,
            "code",
            {
                "operation": "get_document_symbols",
                "file_path": "scripts/agents/worktree-lib.ts",
                "limit": 20,
            },
        )

        metrics["latency_ms"]["code.generate_codebase_overview"] = {
            "avg_ms": round(code_overview_ms, 2)
        }
        metrics["latency_ms"]["code.search_codebase_map"] = {"avg_ms": round(code_map_ms, 2)}
        metrics["latency_ms"]["code.get_document_symbols"] = {
            "avg_ms": round(document_symbols_ms, 2)
        }
        metrics["tokens"]["code.generate_codebase_overview"] = {"avg_tokens": code_overview_tokens}
        metrics["tokens"]["code.search_codebase_map"] = {"avg_tokens": code_map_tokens}
        metrics["tokens"]["code.get_document_symbols"] = {"avg_tokens": document_symbols_tokens}
        metrics["tools"]["code.generate_codebase_overview"] = {
            "total_files": code_overview_result.get("total_files", 0),
            "total_symbols": code_overview_result.get("total_symbols", 0),
        }
        metrics["tools"]["code.search_codebase_map"] = {
            "entries": len(code_map_result.get("entries", []))
        }
        metrics["tools"]["code.get_document_symbols"] = {
            "file": document_symbols_result.get("file"),
            "total": document_symbols_result.get("total", 0),
        }

        commit_history_result, commit_history_ms, commit_history_tokens = await _timed_call(
            mcp,
            "commit_history",
            {"path": str(codebase_path), "limit": 20},
        )
        commit_search_result, commit_search_ms, commit_search_tokens = await _timed_call(
            mcp,
            "commit_search",
            {"query": "agent workflow worktree runtime", "limit": 10},
        )
        metrics["latency_ms"]["commit_history"] = {"avg_ms": round(commit_history_ms, 2)}
        metrics["latency_ms"]["commit_search"] = {"avg_ms": round(commit_search_ms, 2)}
        metrics["tokens"]["commit_history"] = {"avg_tokens": commit_history_tokens}
        metrics["tokens"]["commit_search"] = {"avg_tokens": commit_search_tokens}
        metrics["tools"]["commit_history"] = {
            "total": commit_history_result.get("total", 0),
            "branch": commit_history_result.get("branch"),
        }
        metrics["tools"]["commit_search"] = {"total": commit_search_result.get("total", 0)}

        remember_result, remember_ms, remember_tokens = await _timed_call(
            mcp,
            "remember",
            {
                "content": (
                    "Platform benchmark memory: worktree prep lives in "
                    "scripts/agents/worktree-lib.ts"
                ),
                "memory_type": "note",
                "tags": "benchmark,platform,worktree",
                "project": "platform-benchmark",
                "ttl": "session",
            },
        )
        recall_result, recall_ms, recall_tokens = await _timed_call(
            mcp,
            "recall",
            {
                "query": "where is worktree preparation logic",
                "limit": 5,
                "tags": "benchmark,platform",
            },
        )
        forget_result, forget_ms, forget_tokens = await _timed_call(
            mcp,
            "forget",
            {"memory_id": remember_result.get("id", "")},
        )
        metrics["latency_ms"]["remember"] = {"avg_ms": round(remember_ms, 2)}
        metrics["latency_ms"]["recall"] = {"avg_ms": round(recall_ms, 2)}
        metrics["latency_ms"]["forget"] = {"avg_ms": round(forget_ms, 2)}
        metrics["tokens"]["remember"] = {"avg_tokens": remember_tokens}
        metrics["tokens"]["recall"] = {"avg_tokens": recall_tokens}
        metrics["tokens"]["forget"] = {"avg_tokens": forget_tokens}
        metrics["memory"] = {
            "remember_id": remember_result.get("id"),
            "recall_total": recall_result.get("total", 0),
            "forget_deleted": forget_result.get("deleted_count", 0),
        }

        knowledge_status_result, knowledge_status_ms, knowledge_status_tokens = await _timed_call(
            mcp,
            "knowledge",
            {"command": "status"},
        )
        knowledge_add_result, knowledge_add_ms, knowledge_add_tokens = await _timed_call(
            mcp,
            "knowledge",
            {
                "command": "add",
                "name": "platform-benchmark",
                "value": "Platform benchmark note: prefer bun run agent:proof for proof-of-work.",
            },
        )
        knowledge_search_result, knowledge_search_ms, knowledge_search_tokens = await _timed_call(
            mcp,
            "knowledge",
            {"command": "search", "query": "agent proof of work command", "limit": 5},
        )
        knowledge_remove_result, knowledge_remove_ms, knowledge_remove_tokens = await _timed_call(
            mcp,
            "knowledge",
            {
                "command": "remove",
                "context_id": knowledge_add_result.get("context_id", ""),
            },
        )
        metrics["latency_ms"]["knowledge.status"] = {"avg_ms": round(knowledge_status_ms, 2)}
        metrics["latency_ms"]["knowledge.add"] = {"avg_ms": round(knowledge_add_ms, 2)}
        metrics["latency_ms"]["knowledge.search"] = {"avg_ms": round(knowledge_search_ms, 2)}
        metrics["latency_ms"]["knowledge.remove"] = {"avg_ms": round(knowledge_remove_ms, 2)}
        metrics["tokens"]["knowledge.status"] = {"avg_tokens": knowledge_status_tokens}
        metrics["tokens"]["knowledge.add"] = {"avg_tokens": knowledge_add_tokens}
        metrics["tokens"]["knowledge.search"] = {"avg_tokens": knowledge_search_tokens}
        metrics["tokens"]["knowledge.remove"] = {"avg_tokens": knowledge_remove_tokens}
        metrics["knowledge"] = {
            "contexts_before": knowledge_status_result.get("contexts", 0),
            "context_id": knowledge_add_result.get("context_id"),
            "search_total": knowledge_search_result.get("total", 0),
            "remove_chunks_deleted": knowledge_remove_result.get("chunks_deleted", 0),
        }

        repo_status_result, repo_status_ms, repo_status_tokens = await _timed_call(
            mcp, "repo_status"
        )
        metrics["latency_ms"]["repo_status"] = {"avg_ms": round(repo_status_ms, 2)}
        metrics["tokens"]["repo_status"] = {"avg_tokens": repo_status_tokens}
        metrics["tools"]["repo_status"] = {"total_repos": repo_status_result.get("total_repos", 0)}

        if sandbox_refs:
            retrieve_result, retrieve_ms, retrieve_tokens = await _timed_call(
                mcp,
                "retrieve",
                {"ref_id": sandbox_refs[0]},
            )
            metrics["latency_ms"]["retrieve"] = {"avg_ms": round(retrieve_ms, 2)}
            metrics["tokens"]["retrieve"] = {"avg_tokens": retrieve_tokens}
            metrics["tools"]["retrieve"] = {
                "ref_id": sandbox_refs[0],
                "content_chars": len(str(retrieve_result.get("content", ""))),
            }

        final_status_result, final_status_ms, final_status_tokens = await _timed_call(mcp, "status")
        metrics["status_after"] = final_status_result
        metrics["latency_ms"]["status_after"] = {"avg_ms": round(final_status_ms, 2)}
        metrics["tokens"]["status_after"] = {"avg_tokens": final_status_tokens}

        cache_info = final_status_result.get("cache")
        if isinstance(cache_info, dict):
            metrics["cache"]["status_cache"] = cache_info

        total_tokens = 0
        for value in metrics["tokens"].values():
            if isinstance(value, dict) and "avg_tokens" in value:
                total_tokens += int(value["avg_tokens"])
        metrics["totals"] = {
            "measured_tool_surfaces": len(metrics["tokens"]),
            "sum_avg_tokens": total_tokens,
        }

        return metrics
    finally:
        try:
            state = get_state()
            vector_engine = getattr(state, "vector_engine", None)
            if vector_engine and getattr(vector_engine, "_embedding_service", None):
                vector_engine._embedding_service.unload()
            state.shutdown()
        except Exception:
            pass
        gc.collect()


def main() -> dict:
    parser = argparse.ArgumentParser(description="Run the live platform benchmark.")
    parser.add_argument("--path", type=Path, required=True, help="Codebase path to benchmark")
    parser.add_argument(
        "--storage-dir",
        type=Path,
        required=True,
        help="Fresh storage directory to use for this run",
    )
    parser.add_argument(
        "--index-timeout",
        type=int,
        default=3600,
        help="Seconds to wait for indexing to complete",
    )
    parser.add_argument("--output", type=Path, default=None, help="Optional JSON output path")
    args = parser.parse_args()

    storage_dir = args.storage_dir.resolve()
    _ensure_fresh_storage_dir(storage_dir)

    metrics = asyncio.run(
        run_benchmark(
            codebase_path=args.path.resolve(),
            storage_dir=storage_dir,
            index_timeout=args.index_timeout,
        )
    )
    print(json.dumps(metrics, indent=2))
    if args.output is not None:
        args.output.write_text(json.dumps(metrics, indent=2))
    return metrics


if __name__ == "__main__":
    main()
