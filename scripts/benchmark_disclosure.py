"""Benchmark: Progressive Disclosure & AST Compression Token Savings.

Measures the token reduction achieved by:
1. ToolResponsePolicy (universal progressive disclosure)
2. AST-aware snippet compression in search previews
3. CompactionArchive searchability

Run: PYTHONPATH=src python scripts/benchmark_disclosure.py
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from contextro_mcp.engines.output_sandbox import OutputSandbox
from contextro_mcp.execution.ast_compression import compress_snippet
from contextro_mcp.execution.response_policy import ToolResponsePolicy
from contextro_mcp.memory.compaction_archive import CompactionArchive


def estimate_tokens(data) -> int:
    if isinstance(data, str):
        return len(data) // 4
    return len(json.dumps(data, default=str)) // 4


def benchmark_progressive_disclosure():
    """Measure token savings from ToolResponsePolicy on realistic payloads."""
    sandbox = OutputSandbox(max_entries=100, ttl=600.0)
    policy = ToolResponsePolicy(output_sandbox=sandbox, threshold_tokens=300)

    # Simulate realistic tool responses of varying sizes
    test_cases = [
        # Small response (should pass through)
        {
            "name": "find_callers_small",
            "data": {
                "symbol": "parse",
                "total": 3,
                "callers": [
                    "main (src/app.py:10)",
                    "test_parse (tests/test.py:5)",
                    "cli (src/cli.py:20)",
                ],
            },
        },
        # Medium response (borderline)
        {
            "name": "impact_medium",
            "data": {
                "symbol": "DatabaseConnection",
                "max_depth": 10,
                "total_impacted": 15,
                "impacted_symbols": [
                    f"handler_{i} (src/handlers/h{i}.py:{i * 10})" for i in range(15)
                ],
                "impacted_files": {f"src/handlers/h{i}.py": [f"handler_{i}"] for i in range(15)},
                "impacted_dirs": {"src/handlers": [f"h{i}.py" for i in range(15)]},
            },
        },
        # Large response (should be sandboxed)
        {
            "name": "impact_large",
            "data": {
                "symbol": "BaseModel",
                "max_depth": 10,
                "total_impacted": 50,
                "impacted_symbols": [f"model_{i} (src/models/m{i}.py:{i * 5})" for i in range(50)],
                "impacted_files": {
                    f"src/models/m{i}.py": [f"model_{i}", f"validate_{i}"] for i in range(25)
                },
                "impacted_dirs": {
                    f"src/models": [f"m{i}.py" for i in range(25)],
                    "src/api": [f"endpoint_{i}.py" for i in range(15)],
                },
            },
        },
        # Very large explain response
        {
            "name": "explain_large",
            "data": {
                "symbol": "SearchEngine",
                "type": "class",
                "file": "src/engines/search.py",
                "line": 45,
                "code": "class SearchEngine:\n"
                + "    def method_{i}(self): pass\n".format(i=i) * 30
                if False
                else "class SearchEngine:\n"
                + "\n".join(
                    [f"    def method_{i}(self, arg{i}): return self.data[{i}]" for i in range(30)]
                ),
                "callers": [f"caller_{i} (src/c{i}.py:{i})" for i in range(20)],
                "callees": [f"callee_{i} (src/e{i}.py:{i})" for i in range(15)],
                "related": [
                    {"name": f"related_{i}", "file": f"src/r{i}.py", "score": 0.9 - i * 0.05}
                    for i in range(10)
                ],
            },
        },
    ]

    print("=" * 60)
    print("Progressive Disclosure Benchmark")
    print("=" * 60)
    print(f"{'Case':<25} {'Original':>10} {'After':>10} {'Saved':>8} {'Sandboxed'}")
    print("-" * 70)

    total_original = 0
    total_after = 0

    for case in test_cases:
        original_tokens = estimate_tokens(case["data"])
        result = policy.apply(case["data"], tool_name=case["name"])
        after_tokens = estimate_tokens(result)
        saved = original_tokens - after_tokens
        sandboxed = "sandbox_ref" in result

        total_original += original_tokens
        total_after += after_tokens

        print(
            f"{case['name']:<25} {original_tokens:>10} {after_tokens:>10} {saved:>8} {'YES' if sandboxed else 'no'}"
        )

    total_saved = total_original - total_after
    pct = (total_saved / total_original * 100) if total_original > 0 else 0
    print("-" * 70)
    print(f"{'TOTAL':<25} {total_original:>10} {total_after:>10} {total_saved:>8} ({pct:.1f}%)")
    print()
    return {"original": total_original, "after": total_after, "saved_pct": round(pct, 1)}


def benchmark_ast_compression():
    """Measure token savings from AST-aware snippet compression."""
    test_snippets = [
        {
            "name": "python_function",
            "lang": "python",
            "code": '''def process_batch(items: list[dict], config: Config) -> list[Result]:
    """Process a batch of items according to configuration.

    Args:
        items: List of item dictionaries to process.
        config: Processing configuration.

    Returns:
        List of processed results.
    """
    results = []
    for item in items:
        validated = validate_item(item, config.schema)
        if not validated:
            logger.warning("Invalid item: %s", item.get("id"))
            continue
        transformed = apply_transforms(validated, config.transforms)
        enriched = enrich_with_metadata(transformed, config.metadata_source)
        scored = calculate_relevance_score(enriched, config.scoring_weights)
        if scored.relevance > config.min_relevance:
            results.append(scored)
        else:
            logger.debug("Below threshold: %s (%.3f)", item.get("id"), scored.relevance)
    return sorted(results, key=lambda r: r.relevance, reverse=True)
''',
        },
        {
            "name": "python_class",
            "lang": "python",
            "code": '''class EmbeddingService:
    """Manages embedding model lifecycle and batch inference."""

    def __init__(self, model_name: str = "potion-code-16m", batch_size: int = 128):
        self.model_name = model_name
        self.batch_size = batch_size
        self._model = None
        self._lock = threading.Lock()

    def embed(self, texts: list[str]) -> list[list[float]]:
        """Embed a list of texts into vectors."""
        model = self._get_model()
        results = []
        for i in range(0, len(texts), self.batch_size):
            batch = texts[i:i + self.batch_size]
            embeddings = model.encode(batch)
            results.extend(embeddings.tolist())
        return results

    def _get_model(self):
        if self._model is None:
            with self._lock:
                if self._model is None:
                    self._model = self._load_model()
        return self._model

    def _load_model(self):
        from model2vec import StaticModel
        return StaticModel.from_pretrained(self.model_name)

    def unload(self):
        """Free model memory."""
        self._model = None
        import gc
        gc.collect()
''',
        },
        {
            "name": "javascript_function",
            "lang": "javascript",
            "code": """async function fetchAndProcessResults(query, options = {}) {
  const { limit = 10, timeout = 5000, retries = 3 } = options;

  for (let attempt = 0; attempt < retries; attempt++) {
    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), timeout);

      const response = await fetch(`/api/search?q=${encodeURIComponent(query)}&limit=${limit}`, {
        signal: controller.signal,
        headers: { 'Content-Type': 'application/json' }
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const data = await response.json();
      return data.results.map(r => ({
        id: r.id,
        title: r.title,
        score: r.relevance_score,
        snippet: r.highlighted_text
      }));
    } catch (error) {
      if (attempt === retries - 1) throw error;
      await new Promise(resolve => setTimeout(resolve, 1000 * (attempt + 1)));
    }
  }
}
""",
        },
    ]

    print("=" * 60)
    print("AST-Aware Snippet Compression Benchmark")
    print("=" * 60)
    print(f"{'Snippet':<25} {'Original':>10} {'Compressed':>10} {'Reduction':>10}")
    print("-" * 60)

    total_original = 0
    total_compressed = 0

    for case in test_snippets:
        original = len(case["code"])
        compressed = compress_snippet(case["code"], case["lang"])
        comp_len = len(compressed)

        total_original += original
        total_compressed += comp_len

        reduction = (1 - comp_len / original) * 100 if original > 0 else 0
        print(f"{case['name']:<25} {original:>10} {comp_len:>10} {reduction:>9.1f}%")

    total_reduction = (1 - total_compressed / total_original) * 100 if total_original > 0 else 0
    print("-" * 60)
    print(f"{'TOTAL':<25} {total_original:>10} {total_compressed:>10} {total_reduction:>9.1f}%")
    print()
    return {
        "original_chars": total_original,
        "compressed_chars": total_compressed,
        "reduction_pct": round(total_reduction, 1),
    }


def benchmark_compaction_archive():
    """Measure compaction archive search performance."""
    archive = CompactionArchive(max_entries=20, ttl=86400.0)

    # Simulate archiving a realistic session
    session_content = """
User asked: "How does the authentication flow work?"
Agent searched: "authentication flow" → 5 results
Agent explained: AuthMiddleware (src/middleware/auth.py:23)
  - Validates JWT tokens
  - Checks token expiry
  - Extracts user claims
Agent found callers: 12 callers of validate_token
Agent ran impact analysis: changing AuthMiddleware affects 34 symbols
Key decision: JWT tokens use RS256 with 24h expiry, refresh tokens in Redis
Agent searched: "token refresh logic" → 3 results
Agent modified: src/middleware/auth.py (added refresh endpoint)
Agent ran tests: 45 passed, 0 failed
""".strip()

    ref_id = archive.archive(session_content)

    # Test search performance
    queries = ["JWT", "refresh token", "AuthMiddleware", "Redis", "nonexistent_term"]
    print("=" * 60)
    print("Compaction Archive Search Benchmark")
    print("=" * 60)
    print(f"Archived: {len(session_content)} chars as {ref_id}")
    print(f"{'Query':<25} {'Results':>8} {'Excerpts':>10}")
    print("-" * 50)

    for q in queries:
        results = archive.search(q)
        excerpt_count = sum(len(r["excerpts"]) for r in results)
        print(f"{q:<25} {len(results):>8} {excerpt_count:>10}")

    print()
    return {"archive_size": len(session_content), "ref_id": ref_id}


if __name__ == "__main__":
    print("\n")
    disclosure_results = benchmark_progressive_disclosure()
    ast_results = benchmark_ast_compression()
    archive_results = benchmark_compaction_archive()

    print("=" * 60)
    print("SUMMARY")
    print("=" * 60)
    print(
        f"Progressive Disclosure: {disclosure_results['saved_pct']}% token reduction on large responses"
    )
    print(
        f"AST Compression:        {ast_results['reduction_pct']}% character reduction on code snippets"
    )
    print(f"Compaction Archive:     Searchable ({archive_results['archive_size']} chars archived)")
    print()
