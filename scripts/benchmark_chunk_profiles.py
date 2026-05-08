"""Compare retrieval quality across Contextro chunking profiles."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from benchmark_retrieval_quality import run_benchmark

PROFILES: dict[str, dict[str, str]] = {
    "minimal": {
        "CTX_CHUNK_CONTEXT_MODE": "minimal",
        "CTX_SMART_CHUNK_RELATIONSHIPS_ENABLED": "false",
        "CTX_SMART_CHUNK_FILE_CONTEXT_ENABLED": "false",
    },
    "contextual": {
        "CTX_CHUNK_CONTEXT_MODE": "rich",
        "CTX_SMART_CHUNK_RELATIONSHIPS_ENABLED": "false",
        "CTX_SMART_CHUNK_FILE_CONTEXT_ENABLED": "false",
    },
    "smart": {
        "CTX_CHUNK_CONTEXT_MODE": "rich",
        "CTX_SMART_CHUNK_RELATIONSHIPS_ENABLED": "true",
        "CTX_SMART_CHUNK_FILE_CONTEXT_ENABLED": "true",
    },
}


def main() -> dict:
    parser = argparse.ArgumentParser(
        description="Benchmark retrieval quality across chunk profiles."
    )
    parser.add_argument(
        "--path",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "src",
        help="Codebase path to index and benchmark",
    )
    parser.add_argument(
        "--query-limit",
        type=int,
        default=20,
        help="Maximum number of golden queries to generate from docstrings",
    )
    parser.add_argument(
        "--index-timeout",
        type=int,
        default=180,
        help="Seconds to wait for background indexing before failing",
    )
    parser.add_argument(
        "--profiles",
        nargs="*",
        choices=sorted(PROFILES.keys()),
        default=sorted(PROFILES.keys()),
        help="Which chunk profiles to evaluate",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Optional JSON output path",
    )
    args = parser.parse_args()

    import asyncio

    results: dict[str, dict] = {}
    for profile in args.profiles:
        metrics = asyncio.run(
            run_benchmark(
                args.path.resolve(),
                args.query_limit,
                args.index_timeout,
                env_overrides=PROFILES[profile],
            )
        )
        results[profile] = metrics

    summary = {
        profile: {
            mode: {
                "mrr": data["modes"][mode]["mrr"],
                "recall_at_5": data["modes"][mode]["recall_at_5"],
                "avg_tokens": data["modes"][mode]["avg_tokens"],
            }
            for mode in data.get("modes", {})
        }
        for profile, data in results.items()
    }
    payload = {
        "codebase": str(args.path.resolve()),
        "profiles": results,
        "summary": summary,
    }

    print(json.dumps(payload, indent=2))
    if args.output is not None:
        args.output.write_text(json.dumps(payload, indent=2))

    return payload


if __name__ == "__main__":
    main()
