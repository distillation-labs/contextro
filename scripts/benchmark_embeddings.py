"""
Embedding model benchmark for Contextia — autoresearch style.

Measures three things that matter for code search:
  1. Indexing speed  — tokens/sec during batch embedding (lower = slower indexing)
  2. Search latency  — ms per query embed (lower = faster search)
  3. Retrieval quality — MRR@10 on a hand-crafted code search eval set

The eval set contains 20 (query, expected_function) pairs drawn from this
codebase itself, covering semantic, keyword, and cross-language queries.
Each model indexes the same 50 code chunks, then we measure how often the
correct chunk ranks in the top-10 results.

Usage:
    python scripts/benchmark_embeddings.py
    python scripts/benchmark_embeddings.py --models jina-code nomic-embed bge-small-en
    python scripts/benchmark_embeddings.py --quick   # skip quality eval
"""

import argparse
import sys
import time
from pathlib import Path
from typing import Dict, List, Tuple

# Add src to path
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

# ---------------------------------------------------------------------------
# Eval set: (natural language query, target function/symbol name)
# These are real queries an agent would ask about this codebase.
# ---------------------------------------------------------------------------
EVAL_PAIRS: List[Tuple[str, str]] = [
    # Semantic queries — don't contain the exact function name
    ("how does authentication work", "authenticate"),
    ("find all callers of a function", "get_callers"),
    ("what breaks if I change this symbol", "get_transitive_callers"),
    ("store a memory with expiry", "remember"),
    ("search memories by meaning", "recall"),
    ("detect branch switches in git", "has_changed"),
    ("hash file contents for change detection", "hash_files"),
    ("discover source files respecting gitignore", "discover_files"),
    ("extract commit history from a repository", "extract_commits"),
    ("parallel file mtime scanning", "scan_mtimes"),
    # Keyword-adjacent queries
    ("reciprocal rank fusion of search results", "fuse"),
    ("token budget truncation verbosity levels", "build_explain_response"),
    ("incremental reindex changed files only", "incremental_index"),
    ("graph node degree in degree out degree", "get_node_degree"),
    ("embed batch of texts with ONNX", "embed_batch"),
    # Cross-concept queries
    ("what calls the pipeline index method", "index"),
    ("cross repository context manager", "register_repo"),
    ("real time file watcher debounce", "_handle_file_change"),
    ("code complexity smell detection", "detect_code_smells"),
    ("serialize graph to sqlite for warm start", "save"),
]

# ---------------------------------------------------------------------------
# Corpus: 50 representative code chunks from this codebase
# Each chunk is (symbol_name, text_snippet)
# ---------------------------------------------------------------------------
def build_corpus() -> List[Tuple[str, str]]:
    """Build a corpus of code chunks by parsing key source files."""
    corpus = []
    src_root = Path(__file__).parent.parent / "src" / "contextia_mcp"

    # We'll extract function/class docstrings + signatures from key files
    import ast

    def extract_chunks(filepath: Path) -> List[Tuple[str, str]]:
        chunks = []
        try:
            tree = ast.parse(filepath.read_text())
        except Exception:
            return chunks
        for node in ast.walk(tree):
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                name = node.name
                # Get docstring
                doc = ast.get_docstring(node) or ""
                # Build a text representation
                args = ""
                if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    arg_names = [a.arg for a in node.args.args]
                    args = ", ".join(arg_names)
                kind = "class" if isinstance(node, ast.ClassDef) else "function"
                text = (
                    f"{kind}: {name}({args})\n"
                    f"file: {filepath.relative_to(src_root.parent.parent)}\n"
                    f"line: {node.lineno}\n"
                )
                if doc:
                    text += f"docstring: {doc[:300]}\n"
                chunks.append((name, text))
        return chunks

    # Key files to index
    key_files = [
        src_root / "server.py",
        src_root / "indexing" / "pipeline.py",
        src_root / "engines" / "graph_engine.py",
        src_root / "engines" / "fusion.py",
        src_root / "memory" / "memory_store.py",
        src_root / "git" / "commit_indexer.py",
        src_root / "git" / "branch_watcher.py",
        src_root / "git" / "cross_repo.py",
        src_root / "accelerator.py",
        src_root / "indexing" / "embedding_service.py",
        src_root / "analysis" / "code_analyzer.py",
        src_root / "persistence" / "store.py",
        src_root / "formatting" / "response_builder.py",
    ]

    seen = set()
    for f in key_files:
        if f.exists():
            for name, text in extract_chunks(f):
                if name not in seen:
                    seen.add(name)
                    corpus.append((name, text))
                if len(corpus) >= 80:
                    break
        if len(corpus) >= 80:
            break

    return corpus[:80]


# ---------------------------------------------------------------------------
# Benchmark runner
# ---------------------------------------------------------------------------

def cosine_sim(a: List[float], b: List[float]) -> float:
    dot = sum(x * y for x, y in zip(a, b))
    na = sum(x * x for x in a) ** 0.5
    nb = sum(x * x for x in b) ** 0.5
    if na == 0 or nb == 0:
        return 0.0
    return dot / (na * nb)


def run_benchmark(model_key: str, corpus: List[Tuple[str, str]], quick: bool = False) -> Dict:
    """Run full benchmark for one model. Returns results dict."""
    from contextia_mcp.indexing.embedding_service import EMBEDDING_MODELS, EmbeddingService

    if model_key not in EMBEDDING_MODELS:
        return {"error": f"Model '{model_key}' not in EMBEDDING_MODELS"}

    cfg = EMBEDDING_MODELS[model_key]
    print(f"\n{'='*60}")
    print(f"  Model: {model_key}")
    print(f"  HF:    {cfg['hf_name']}")
    print(f"  Dims:  {cfg['dimensions']}")
    print(f"{'='*60}")

    try:
        svc = EmbeddingService(model_key, device="cpu", batch_size=32)
        # Warm up / load model — force trust_remote_code for benchmarking
        print("  Loading model...", end="", flush=True)
        t_load = time.perf_counter()
        # Patch settings to allow trust_remote_code during benchmark
        import os
        os.environ["CTX_TRUST_REMOTE_CODE"] = "true"
        from contextia_mcp.config import reset_settings
        reset_settings()
        svc._load_model()
        load_time = time.perf_counter() - t_load
        print(f" {load_time:.1f}s")
    except Exception as e:
        print(f"  FAILED to load: {e}")
        return {"model": model_key, "error": str(e)}

    results = {
        "model": model_key,
        "hf_name": cfg["hf_name"],
        "dimensions": cfg["dimensions"],
        "load_time_s": round(load_time, 2),
    }

    # --- 1. Indexing speed ---
    texts = [text for _, text in corpus]
    print(f"  Indexing speed ({len(texts)} chunks)...", end="", flush=True)
    t0 = time.perf_counter()
    corpus_vecs = svc.embed_batch(texts, batch_size=32)
    index_time = time.perf_counter() - t0
    total_tokens = sum(len(t.split()) for t in texts)  # approx
    tok_per_sec = total_tokens / index_time
    results["index_time_s"] = round(index_time, 2)
    results["approx_tokens_per_sec"] = round(tok_per_sec)
    print(f" {index_time:.2f}s  (~{tok_per_sec:.0f} tok/s)")

    # --- 2. Search latency ---
    print("  Search latency (100 queries)...", end="", flush=True)
    query_times = []
    for _ in range(100):
        t0 = time.perf_counter()
        svc.embed("how does authentication work", is_query=True)
        query_times.append(time.perf_counter() - t0)
    # First call loads cache; use median of last 90
    median_ms = sorted(query_times[10:])[45] * 1000
    results["query_latency_ms"] = round(median_ms, 2)
    print(f" {median_ms:.1f}ms median")

    if quick:
        svc.unload()
        return results

    # --- 3. Retrieval quality: MRR@10 ---
    print(f"  Retrieval quality (MRR@10 on {len(EVAL_PAIRS)} queries)...", end="", flush=True)
    corpus_names = [name for name, _ in corpus]

    reciprocal_ranks = []
    hits_at_1 = 0
    hits_at_5 = 0
    hits_at_10 = 0
    misses = []

    for query, target_name in EVAL_PAIRS:
        q_vec = svc.embed(query, is_query=True)
        scores = [(cosine_sim(q_vec, cv), cn) for cv, cn in zip(corpus_vecs, corpus_names)]
        scores.sort(reverse=True)
        top10 = [name for _, name in scores[:10]]

        # Find rank of target (exact or substring match)
        rank = None
        for i, name in enumerate(top10, 1):
            if name == target_name or target_name in name or name in target_name:
                rank = i
                break

        if rank is not None:
            reciprocal_ranks.append(1.0 / rank)
            if rank == 1:
                hits_at_1 += 1
            if rank <= 5:
                hits_at_5 += 1
            hits_at_10 += 1
        else:
            reciprocal_ranks.append(0.0)
            misses.append((query, target_name, [s[1] for s in scores[:3]]))

    mrr = sum(reciprocal_ranks) / len(reciprocal_ranks)
    results["mrr_at_10"] = round(mrr, 4)
    results["hits_at_1"] = hits_at_1
    results["hits_at_5"] = hits_at_5
    results["hits_at_10"] = hits_at_10
    results["total_queries"] = len(EVAL_PAIRS)
    print(f" MRR@10={mrr:.3f}  H@1={hits_at_1}  H@5={hits_at_5}  H@10={hits_at_10}")

    if misses:
        print(f"  Misses ({len(misses)}):")
        for q, t, top3 in misses[:5]:
            print(f"    query='{q}' target='{t}' got={top3}")

    svc.unload()
    return results


def print_summary(all_results: List[Dict]) -> None:
    """Print a comparison table and declare a winner."""
    valid = [r for r in all_results if "error" not in r]
    if not valid:
        print("\nNo valid results.")
        return

    print("\n" + "=" * 80)
    print("RESULTS SUMMARY")
    print("=" * 80)
    print(f"{'Model':<28} {'Dims':>5} {'Load':>6} {'Idx(s)':>7} {'tok/s':>7} {'Q(ms)':>7} {'MRR@10':>7} {'H@1':>4} {'H@10':>5}")
    print("-" * 80)

    for r in valid:
        mrr = r.get("mrr_at_10", "-")
        h1 = r.get("hits_at_1", "-")
        h10 = r.get("hits_at_10", "-")
        print(
            f"{r['model']:<28} "
            f"{r['dimensions']:>5} "
            f"{r['load_time_s']:>6.1f}s "
            f"{r['index_time_s']:>6.2f}s "
            f"{r['approx_tokens_per_sec']:>7} "
            f"{r['query_latency_ms']:>6.1f}ms "
            f"{str(mrr):>7} "
            f"{str(h1):>4} "
            f"{str(h10):>5}"
        )

    print("=" * 80)

    # Declare winner on MRR@10 (quality), then speed as tiebreaker
    if all(r.get("mrr_at_10") is not None for r in valid):
        winner = max(valid, key=lambda r: (r["mrr_at_10"], r["approx_tokens_per_sec"]))
        fastest = max(valid, key=lambda r: r["approx_tokens_per_sec"])
        print(f"\n🏆 Best quality:  {winner['model']}  (MRR@10={winner['mrr_at_10']})")
        print(f"⚡ Fastest index: {fastest['model']}  ({fastest['approx_tokens_per_sec']} tok/s)")
        if winner["model"] == fastest["model"]:
            print(f"✅ Same model wins on both — clear winner: {winner['model']}")
        else:
            # Compute quality gap
            mrrs = {r["model"]: r["mrr_at_10"] for r in valid}
            gap = mrrs[winner["model"]] - mrrs[fastest["model"]]
            speed_ratio = fastest["approx_tokens_per_sec"] / winner["approx_tokens_per_sec"]
            print(f"\n   Quality gap: {gap:.3f} MRR points")
            print(f"   Speed ratio: {speed_ratio:.1f}x faster for {fastest['model']}")
            if gap < 0.05:
                print(f"   → Gap < 0.05: speed wins → recommend {fastest['model']}")
            else:
                print(f"   → Gap ≥ 0.05: quality wins → recommend {winner['model']}")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Contextia embedding model benchmark")
    parser.add_argument(
        "--models", nargs="+",
        default=["jina-code", "nomic-embed", "bge-small-en"],
        help="Models to benchmark",
    )
    parser.add_argument("--quick", action="store_true", help="Skip quality eval")
    args = parser.parse_args()

    print("Building corpus from codebase...")
    corpus = build_corpus()
    print(f"Corpus: {len(corpus)} chunks")

    all_results = []
    for model_key in args.models:
        result = run_benchmark(model_key, corpus, quick=args.quick)
        all_results.append(result)

    print_summary(all_results)

    # Save results
    import json
    out = Path(__file__).parent / "benchmark_results.json"
    out.write_text(json.dumps(all_results, indent=2))
    print(f"\nResults saved to {out}")


if __name__ == "__main__":
    main()
