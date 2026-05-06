"""Benchmark Contextia token efficiency against the browser-use codebase.

This uses a real 65k-line Python project (179 files) to measure
actual token output from realistic coding agent queries.

Metric: total_output_tokens (lower is better)
"""

import json
import os
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from benchmark_utils import (
    benchmark_session,
    call_tool,
    estimate_tokens,
    index_codebase,
    sync_vector_engine,
)

_browser_use_path = os.environ.get("BROWSER_USE_PATH")
if not _browser_use_path:
    raise RuntimeError("Set BROWSER_USE_PATH to the browser-use repository path.")
BROWSER_USE_PATH = Path(_browser_use_path).expanduser()


async def run_benchmark() -> dict:
    import tempfile

    tmp_dir = Path(tempfile.mkdtemp(prefix="ctx_bench_bu_"))
    storage_dir = tmp_dir / ".contextia"
    storage_dir.mkdir()

    metrics = {
        "timestamp": time.time(),
        "codebase": "browser-use",
        "tokens_per_search": 0,
        "tokens_per_explain": 0,
        "tokens_per_find_symbol": 0,
        "tokens_per_find_callers": 0,
        "tokens_per_status": 0,
        "tokens_per_impact": 0,
        "total_output_tokens": 0,
        "search_results_count": 0,
        "cache_hit_rate": 0.0,
        "workflow_tool_calls": 0,
    }

    with benchmark_session(storage_dir, dims=384) as (mcp, mock_svc, _server_module):
        from contextia_mcp.config import get_settings
        from contextia_mcp.state import get_state

        settings = get_settings()
        print(
            f"  DEBUG: storage={settings.storage_path}, "
            f"exists={settings.storage_path.exists()}"
        )
        assert str(storage_dir) in str(settings.lancedb_path), (
            f"Storage mismatch: {settings.lancedb_path}"
        )

        # 1. Index
        print("  Indexing browser-use...")
        t0 = time.time()
        index_result = await index_codebase(mcp, _server_module, str(BROWSER_USE_PATH))
        print(f"  Indexed in {time.time() - t0:.1f}s, result: {str(index_result)[:200]}")
        metrics["total_output_tokens"] += estimate_tokens(index_result)
        metrics["workflow_tool_calls"] += 1

        state = get_state()
        sync_vector_engine(state, mock_svc)

        # 2. Status
        status_result = await call_tool(mcp, "status")
        metrics["tokens_per_status"] = estimate_tokens(status_result)
        metrics["total_output_tokens"] += metrics["tokens_per_status"]
        metrics["workflow_tool_calls"] += 1

        # 3. Search queries
        search_queries = [
            "browser page navigation",
            "click element selector",
            "extract text from page",
            "handle authentication login",
            "screenshot capture",
            "wait for element to load",
            "form input fill submit",
            "error handling retry logic",
            "DOM extraction parsing",
            "agent action execution",
        ]
        search_token_total = 0
        for q in search_queries:
            result = await call_tool(mcp, "search", {"query": q, "limit": 10})
            tokens = estimate_tokens(result)
            search_token_total += tokens
            metrics["total_output_tokens"] += tokens
            metrics["workflow_tool_calls"] += 1
            metrics["search_results_count"] += result.get("total", 0)
        metrics["tokens_per_search"] = search_token_total // len(search_queries)

        # 4. Find symbol
        symbol_queries = ["Browser", "Agent", "Controller", "DOMService", "ActionModel"]
        symbol_token_total = 0
        for s in symbol_queries:
            result = await call_tool(mcp, "find_symbol", {"name": s})
            tokens = estimate_tokens(result)
            symbol_token_total += tokens
            metrics["total_output_tokens"] += tokens
            metrics["workflow_tool_calls"] += 1
        metrics["tokens_per_find_symbol"] = symbol_token_total // len(symbol_queries)

        # 5. Find callers
        caller_queries = ["click", "navigate", "extract_content", "get_element"]
        caller_token_total = 0
        for c in caller_queries:
            result = await call_tool(mcp, "find_callers", {"symbol_name": c})
            tokens = estimate_tokens(result)
            caller_token_total += tokens
            metrics["total_output_tokens"] += tokens
            metrics["workflow_tool_calls"] += 1
        metrics["tokens_per_find_callers"] = caller_token_total // len(caller_queries)

        # 6. Explain
        try:
            explain_result = await call_tool(mcp, "explain", {"symbol_name": "Browser"})
            metrics["tokens_per_explain"] = estimate_tokens(explain_result)
            metrics["total_output_tokens"] += metrics["tokens_per_explain"]
            metrics["workflow_tool_calls"] += 1
        except Exception:
            pass

        # 7. Impact
        try:
            impact_result = await call_tool(mcp, "impact", {"symbol_name": "click"})
            metrics["tokens_per_impact"] = estimate_tokens(impact_result)
            metrics["total_output_tokens"] += metrics["tokens_per_impact"]
            metrics["workflow_tool_calls"] += 1
        except Exception:
            pass

        # 8. Repeat searches (cache)
        for q in ["browser page navigation", "click element selector", "extract text from page"]:
            result = await call_tool(mcp, "search", {"query": q, "limit": 10})
            metrics["total_output_tokens"] += estimate_tokens(result)
            metrics["workflow_tool_calls"] += 1

        if hasattr(state, "_query_cache") and state._query_cache:
            metrics["cache_hit_rate"] = round(state._query_cache.hit_rate, 4)

    import shutil
    shutil.rmtree(tmp_dir, ignore_errors=True)
    return metrics


def main():
    import asyncio
    print("=" * 60)
    print("Contextia Token Benchmark — browser-use (65k LOC)")
    print("=" * 60)
    metrics = asyncio.run(run_benchmark())
    print(f"\n{'Metric':<30} {'Value':>15}")
    print("-" * 47)
    for key, value in sorted(metrics.items()):
        if key == "timestamp":
            continue
        if isinstance(value, float):
            print(f"{key:<30} {value:>15.4f}")
        else:
            print(f"{key:<30} {value:>15}")
    print("-" * 47)
    print(f"{'TOTAL OUTPUT TOKENS':<30} {metrics['total_output_tokens']:>15}")
    avg_tokens = metrics["total_output_tokens"] // max(1, metrics["workflow_tool_calls"])
    print(f"{'AVG TOKENS/CALL':<30} {avg_tokens:>15}")
    print("=" * 60)
    results_path = Path(__file__).parent / "benchmark_browser_use_results.json"
    with open(results_path, "w") as f:
        json.dump(metrics, f, indent=2)
    return metrics


if __name__ == "__main__":
    main()
