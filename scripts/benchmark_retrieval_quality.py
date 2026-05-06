"""Retrieval-quality benchmark for Contextia search."""

from __future__ import annotations

import argparse
import ast
import json
import sys
import tempfile
import time
from pathlib import Path

# Add src to path
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from benchmark_utils import (
    benchmark_session,
    call_tool,
    estimate_tokens,
    index_codebase,
    sync_vector_engine,
)


def _first_sentence(text: str) -> str:
    sentence = text.strip().split(".")[0].strip()
    return sentence.rstrip(":")


def collect_docstring_queries(codebase_path: Path, limit: int) -> list[dict[str, str]]:
    """Generate golden queries from Python docstrings in a codebase."""
    queries: list[dict[str, str]] = []

    for file_path in sorted(codebase_path.rglob("*.py")):
        try:
            module = ast.parse(file_path.read_text())
        except (OSError, SyntaxError, UnicodeDecodeError):
            continue

        for node in ast.walk(module):
            if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                continue

            docstring = ast.get_docstring(node)
            if not docstring:
                continue

            query = _first_sentence(docstring)
            if len(query) < 12:
                continue

            queries.append(
                {
                    "query": query,
                    "expected_file": str(file_path.relative_to(codebase_path)),
                    "expected_symbol": node.name,
                }
            )
            if len(queries) >= limit:
                return queries

    return queries


def recall_at_k(results: list[dict], expected: dict[str, str], k: int) -> bool:
    """Return True when the expected file or symbol is present in the top-k results."""
    top_results = results[:k]
    for result in top_results:
        if result.get("name") == expected["expected_symbol"]:
            return True
        if result.get("file") == expected["expected_file"]:
            return True
    return False


def reciprocal_rank(results: list[dict], expected: dict[str, str]) -> float:
    """Return the reciprocal rank of the first matching result."""
    for index, result in enumerate(results, start=1):
        if result.get("name") == expected["expected_symbol"]:
            return 1.0 / index
        if result.get("file") == expected["expected_file"]:
            return 1.0 / index
    return 0.0


async def run_benchmark(codebase_path: Path, query_limit: int, timeout_seconds: int) -> dict:
    """Run retrieval-quality benchmarks across search modes."""
    tmp_dir = Path(tempfile.mkdtemp(prefix="ctx_retrieval_"))
    storage_dir = tmp_dir / ".contextia"
    storage_dir.mkdir()

    queries = collect_docstring_queries(codebase_path, limit=query_limit)
    if not queries:
        raise RuntimeError(f"No docstring queries could be generated from {codebase_path}")

    metrics = {
        "codebase": str(codebase_path),
        "queries": len(queries),
        "modes": {},
    }

    with benchmark_session(storage_dir, dims=384) as (mcp, mock_svc, server_module):
        index_result = await index_codebase(
            mcp,
            server_module,
            str(codebase_path),
            timeout_seconds=timeout_seconds,
        )
        metrics["index"] = {
            "total_files": index_result.get("total_files", 0),
            "total_chunks": index_result.get("total_chunks", 0),
            "time_seconds": index_result.get("time_seconds", 0),
        }

        from contextia_mcp.state import get_state

        state = get_state()
        sync_vector_engine(state, mock_svc)

        for mode in ("hybrid", "vector", "bm25"):
            recalls = {1: 0, 3: 0, 5: 0}
            rr_total = 0.0
            latency_total = 0.0
            token_total = 0
            sandboxed = 0

            for query_case in queries:
                start = time.monotonic()
                result = await call_tool(
                    mcp,
                    "search",
                    {
                        "query": query_case["query"],
                        "mode": mode,
                        "limit": 5,
                        "language": "python",
                    },
                )
                latency_total += (time.monotonic() - start) * 1000
                token_total += estimate_tokens(result)
                sandboxed += int(bool(result.get("sandboxed")))

                results = result.get("results", [])
                for k in recalls:
                    recalls[k] += int(recall_at_k(results, query_case, k))
                rr_total += reciprocal_rank(results, query_case)

            query_count = len(queries)
            metrics["modes"][mode] = {
                "recall_at_1": round(recalls[1] / query_count, 3),
                "recall_at_3": round(recalls[3] / query_count, 3),
                "recall_at_5": round(recalls[5] / query_count, 3),
                "mrr": round(rr_total / query_count, 3),
                "avg_latency_ms": round(latency_total / query_count, 2),
                "avg_tokens": round(token_total / query_count, 1),
                "sandbox_rate": round(sandboxed / query_count, 3),
            }

    return metrics


def main() -> dict:
    parser = argparse.ArgumentParser(description="Run retrieval-quality benchmarks for Contextia.")
    parser.add_argument(
        "--path",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "src",
        help="Codebase path to index and benchmark",
    )
    parser.add_argument(
        "--query-limit",
        type=int,
        default=40,
        help="Maximum number of golden queries to generate from docstrings",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Optional JSON output path",
    )
    parser.add_argument(
        "--index-timeout",
        type=int,
        default=180,
        help="Seconds to wait for background indexing before failing",
    )
    args = parser.parse_args()

    import asyncio

    metrics = asyncio.run(
        run_benchmark(args.path.resolve(), args.query_limit, args.index_timeout)
    )
    print(json.dumps(metrics, indent=2))

    if args.output is not None:
        args.output.write_text(json.dumps(metrics, indent=2))

    return metrics


if __name__ == "__main__":
    main()
