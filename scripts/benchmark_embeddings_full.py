"""
Full autoresearch-style embedding benchmark for Contextia.

Indexes the ENTIRE Contextia codebase (not just 80 chunks), runs 50 diverse
search queries, measures:
  1. Indexing throughput (tokens/sec)
  2. Query latency (ms per query)
  3. MRR@10 retrieval quality
  4. Recall@5 and Recall@10

Each model gets the same corpus, same queries, same eval. Winner is declared
based on a weighted score: 0.6*quality + 0.4*speed (normalized).

Usage:
    python scripts/benchmark_embeddings_full.py
"""

import ast
import json
import os
import sys
import time
from pathlib import Path
from typing import Dict, List, Tuple

sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

# Force trust_remote_code for all models during benchmark
os.environ["CTX_TRUST_REMOTE_CODE"] = "true"

# ---------------------------------------------------------------------------
# 50 eval queries — diverse, realistic, covering semantic + keyword + structural
# ---------------------------------------------------------------------------
EVAL_QUERIES: List[Tuple[str, List[str]]] = [
    # --- Semantic queries (no exact keyword match) ---
    ("how does the server authenticate requests", ["_guard", "_check_tool_permission"]),
    ("find all functions that call another function", ["get_callers", "find_callers"]),
    ("what happens when I change a symbol", ["get_transitive_callers", "impact"]),
    ("store information for later retrieval", ["remember", "_get_memory_store"]),
    ("search stored knowledge by meaning", ["recall"]),
    ("detect when git branch changes", ["has_changed", "BranchState"]),
    ("compute file fingerprints for change detection", ["hash_files", "hash_files_fast"]),
    ("walk directory tree finding source files", ["discover_files", "discover_files_fast"]),
    ("get commit messages from git history", ["extract_commits", "commit_history"]),
    ("check file modification times in parallel", ["scan_mtimes", "scan_mtimes_fast"]),
    ("combine results from multiple search engines", ["fuse", "ReciprocalRankFusion"]),
    ("limit response size based on token budget", ["TokenBudget", "build_explain_response"]),
    ("only reprocess files that changed since last run", ["incremental_index"]),
    ("count incoming and outgoing edges for a node", ["get_node_degree"]),
    ("generate vector representations of text", ["embed_batch", "embed"]),
    ("register a new repository for unified search", ["register_repo", "repo_add"]),
    ("monitor filesystem for code changes", ["DebouncedFileWatcher", "_handle_file_change"]),
    ("analyze code complexity and find smells", ["detect_code_smells", "analyze_complexity"]),
    ("persist graph structure to disk", ["save", "GraphPersistence"]),
    ("explain what a function does with full context", ["explain"]),
    # --- Keyword-adjacent queries ---
    ("reciprocal rank fusion algorithm", ["fuse", "ReciprocalRankFusion"]),
    ("BM25 full text search engine", ["LanceDBBM25Engine", "ensure_fts_index"]),
    ("rustworkx directed graph", ["RustworkxCodeGraph", "add_node"]),
    ("tree-sitter symbol extraction", ["TreeSitterParser", "_extract_symbols"]),
    ("ast-grep structural patterns", ["AstGrepParser", "_extract_functions"]),
    ("LanceDB vector storage", ["LanceDBVectorEngine", "add"]),
    ("FlashRank reranker", ["FlashReranker", "rerank"]),
    ("ONNX runtime inference", ["EmbeddingService", "_load_model"]),
    ("SQLite graph persistence", ["GraphPersistence", "save"]),
    ("watchdog file system events", ["DebouncedFileWatcher", "_EventHandler"]),
    # --- Cross-concept queries ---
    ("how does hybrid search combine vector and keyword results", ["fuse", "search"]),
    ("what is the indexing pipeline flow from files to vectors", ["index", "IndexingPipeline"]),
    ("how are code chunks created from parsed symbols", ["create_chunk", "create_chunks"]),
    ("how does the graph engine find callers of a function", ["get_callers", "_get_callers_unlocked"]),
    ("what happens during server shutdown", ["shutdown"]),
    ("how does incremental indexing detect changed files", ["incremental_index", "_load_metadata"]),
    ("how are memories stored and retrieved", ["remember", "recall", "MemoryStore"]),
    ("what tools are available for code analysis", ["analyze", "architecture", "overview"]),
    ("how does the commit search find relevant commits", ["commit_search", "search_commits"]),
    ("how does cross-repo context work", ["CrossRepoManager", "repo_add", "register_repo"]),
    # --- Specific function lookups ---
    ("the main entry point of the server", ["main", "create_server"]),
    ("validate a file path for security", ["_validate_path"]),
    ("serialize a graph node to JSON", ["_serialize_node"]),
    ("create deterministic chunk IDs", ["_generate_chunk_id", "create_chunk"]),
    ("unload embedding model to free memory", ["unload"]),
    ("get server health and uptime", ["health"]),
    ("delete memories by tag or type", ["forget"]),
    ("find nodes by name in the graph", ["find_nodes_by_name"]),
    ("get project architecture layers and hubs", ["architecture"]),
    ("smart chunking with relationship context", ["create_smart_chunks", "create_relationship_chunks"]),
]


def build_full_corpus(src_root: Path) -> List[Tuple[str, str]]:
    """Parse ALL Python files in src/ to build a comprehensive corpus."""
    corpus = []
    seen_names = set()

    for py_file in sorted(src_root.rglob("*.py")):
        if "__pycache__" in str(py_file):
            continue
        try:
            tree = ast.parse(py_file.read_text())
        except Exception:
            continue

        for node in ast.walk(tree):
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                name = node.name
                if name.startswith("__") and name.endswith("__"):
                    continue  # skip dunder methods
                if name in seen_names:
                    # Allow duplicates with different file context
                    pass

                doc = ast.get_docstring(node) or ""
                args = ""
                if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    arg_names = [a.arg for a in node.args.args]
                    args = ", ".join(arg_names)

                kind = "class" if isinstance(node, ast.ClassDef) else "function"
                rel_path = py_file.relative_to(src_root.parent.parent)

                text = f"{kind}: {name}({args})\nfile: {rel_path}\nline: {node.lineno}\n"
                if doc:
                    text += f"docstring: {doc[:500]}\n"

                # Add body preview for functions (first 5 lines)
                if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    try:
                        source_lines = py_file.read_text().splitlines()
                        body_start = node.lineno  # 1-indexed, body starts after def line
                        body_lines = source_lines[body_start:body_start + 5]
                        if body_lines:
                            text += "body:\n" + "\n".join(body_lines) + "\n"
                    except Exception:
                        pass

                corpus.append((name, text))
                seen_names.add(name)

    return corpus


def cosine_sim(a: List[float], b: List[float]) -> float:
    dot = sum(x * y for x, y in zip(a, b))
    na = sum(x * x for x in a) ** 0.5
    nb = sum(x * x for x in b) ** 0.5
    if na == 0 or nb == 0:
        return 0.0
    return dot / (na * nb)


def run_full_benchmark(model_key: str, corpus: List[Tuple[str, str]]) -> Dict:
    """Run comprehensive benchmark for one model."""
    from contextia_mcp.config import reset_settings
    from contextia_mcp.indexing.embedding_service import EMBEDDING_MODELS, EmbeddingService

    reset_settings()

    if model_key not in EMBEDDING_MODELS:
        return {"model": model_key, "error": "Not in EMBEDDING_MODELS"}

    cfg = EMBEDDING_MODELS[model_key]
    print(f"\n{'='*70}")
    print(f"  Model: {model_key}")
    print(f"  HF:    {cfg['hf_name']}")
    print(f"  Dims:  {cfg['dimensions']}, Context: {cfg['max_seq_length']}")
    print(f"  trust_remote_code: {cfg['trust_remote_code']}")
    print(f"{'='*70}")

    try:
        svc = EmbeddingService(model_key, device="cpu", batch_size=32)
        print("  Loading model...", end="", flush=True)
        t_load = time.perf_counter()
        svc._load_model()
        load_time = time.perf_counter() - t_load
        print(f" {load_time:.1f}s")
    except Exception as e:
        print(f"  FAILED: {e}")
        return {"model": model_key, "error": str(e)}

    results = {
        "model": model_key,
        "hf_name": cfg["hf_name"],
        "dimensions": cfg["dimensions"],
        "trust_remote_code": cfg["trust_remote_code"],
        "load_time_s": round(load_time, 2),
    }

    # --- 1. Indexing speed (full corpus) ---
    texts = [text for _, text in corpus]
    total_chars = sum(len(t) for t in texts)
    total_tokens_approx = sum(len(t.split()) for t in texts)

    print(f"  Indexing {len(texts)} chunks ({total_chars:,} chars, ~{total_tokens_approx:,} tokens)...", end="", flush=True)
    t0 = time.perf_counter()
    corpus_vecs = svc.embed_batch(texts, batch_size=32)
    index_time = time.perf_counter() - t0
    tok_per_sec = total_tokens_approx / index_time
    results["corpus_size"] = len(texts)
    results["index_time_s"] = round(index_time, 3)
    results["tokens_per_sec"] = round(tok_per_sec)
    results["chars_per_sec"] = round(total_chars / index_time)
    print(f" {index_time:.2f}s ({tok_per_sec:.0f} tok/s)")

    # --- 2. Query latency (50 queries, measure each) ---
    print(f"  Query latency ({len(EVAL_QUERIES)} queries x 3 runs)...", end="", flush=True)
    query_texts = [q for q, _ in EVAL_QUERIES]

    # Warm up
    for q in query_texts[:5]:
        svc.embed(q, is_query=True)

    # Measure
    latencies = []
    for _ in range(3):  # 3 runs for stability
        for q in query_texts:
            t0 = time.perf_counter()
            svc.embed(q, is_query=True)
            latencies.append((time.perf_counter() - t0) * 1000)

    latencies.sort()
    p50 = latencies[len(latencies) // 2]
    p95 = latencies[int(len(latencies) * 0.95)]
    avg = sum(latencies) / len(latencies)
    results["query_latency_p50_ms"] = round(p50, 2)
    results["query_latency_p95_ms"] = round(p95, 2)
    results["query_latency_avg_ms"] = round(avg, 2)
    print(f" p50={p50:.1f}ms  p95={p95:.1f}ms  avg={avg:.1f}ms")

    # --- 3. Retrieval quality ---
    print(f"  Retrieval quality ({len(EVAL_QUERIES)} queries)...", end="", flush=True)
    corpus_names = [name for name, _ in corpus]

    reciprocal_ranks = []
    hits_at_1 = 0
    hits_at_3 = 0
    hits_at_5 = 0
    hits_at_10 = 0
    misses = []

    for query, target_names in EVAL_QUERIES:
        q_vec = svc.embed(query, is_query=True)
        scores = [(cosine_sim(q_vec, cv), cn) for cv, cn in zip(corpus_vecs, corpus_names)]
        scores.sort(reverse=True)
        top10_names = [name for _, name in scores[:10]]

        # Find best rank among any of the acceptable targets
        best_rank = None
        for target in target_names:
            for i, name in enumerate(top10_names, 1):
                if name == target or target in name or name in target:
                    if best_rank is None or i < best_rank:
                        best_rank = i
                    break

        if best_rank is not None:
            reciprocal_ranks.append(1.0 / best_rank)
            if best_rank == 1:
                hits_at_1 += 1
            if best_rank <= 3:
                hits_at_3 += 1
            if best_rank <= 5:
                hits_at_5 += 1
            hits_at_10 += 1
        else:
            reciprocal_ranks.append(0.0)
            misses.append((query, target_names, top10_names[:5]))

    mrr = sum(reciprocal_ranks) / len(reciprocal_ranks)
    results["mrr_at_10"] = round(mrr, 4)
    results["hits_at_1"] = hits_at_1
    results["hits_at_3"] = hits_at_3
    results["hits_at_5"] = hits_at_5
    results["hits_at_10"] = hits_at_10
    results["total_queries"] = len(EVAL_QUERIES)
    results["recall_at_5"] = round(hits_at_5 / len(EVAL_QUERIES), 3)
    results["recall_at_10"] = round(hits_at_10 / len(EVAL_QUERIES), 3)
    print(f" MRR@10={mrr:.3f}  R@5={hits_at_5}/{len(EVAL_QUERIES)}  R@10={hits_at_10}/{len(EVAL_QUERIES)}")

    if misses:
        print(f"  Misses ({len(misses)}):")
        for q, targets, top5 in misses[:8]:
            print(f"    Q: '{q[:50]}...' wanted={targets} got={top5[:3]}")

    svc.unload()
    return results


def print_final_results(all_results: List[Dict]) -> str:
    """Print comparison table and declare winner. Returns winner model key."""
    valid = [r for r in all_results if "error" not in r]
    if not valid:
        print("\nNo valid results!")
        return ""

    print("\n" + "=" * 90)
    print("FINAL RESULTS — AUTORESEARCH BENCHMARK")
    print("=" * 90)
    header = (
        f"{'Model':<25} {'Dims':>5} {'Load':>6} {'Idx':>6} {'tok/s':>7} "
        f"{'Q_p50':>6} {'MRR@10':>7} {'R@5':>5} {'R@10':>5} {'trust_rc':>8}"
    )
    print(header)
    print("-" * 90)

    for r in valid:
        print(
            f"{r['model']:<25} "
            f"{r['dimensions']:>5} "
            f"{r['load_time_s']:>5.1f}s "
            f"{r['index_time_s']:>5.2f}s "
            f"{r['tokens_per_sec']:>7} "
            f"{r['query_latency_p50_ms']:>5.1f}ms "
            f"{r['mrr_at_10']:>7.4f} "
            f"{r['recall_at_5']:>5.3f} "
            f"{r['recall_at_10']:>5.3f} "
            f"{'YES' if r['trust_remote_code'] else 'no':>8}"
        )

    print("=" * 90)

    # --- Scoring: 0.6 * quality + 0.4 * speed (normalized) ---
    max_mrr = max(r["mrr_at_10"] for r in valid)
    max_tps = max(r["tokens_per_sec"] for r in valid)

    print("\nWeighted Score (0.6 * quality + 0.4 * speed, normalized):")
    scores = {}
    for r in valid:
        q_norm = r["mrr_at_10"] / max_mrr if max_mrr > 0 else 0
        s_norm = r["tokens_per_sec"] / max_tps if max_tps > 0 else 0
        score = 0.6 * q_norm + 0.4 * s_norm
        scores[r["model"]] = score
        print(f"  {r['model']:<25} quality={q_norm:.3f}  speed={s_norm:.3f}  → score={score:.3f}")

    winner = max(scores, key=scores.get)
    print(f"\n🏆 WINNER: {winner}  (score={scores[winner]:.3f})")

    # Also note if winner requires trust_remote_code
    winner_r = next(r for r in valid if r["model"] == winner)
    if winner_r["trust_remote_code"]:
        # Find best model WITHOUT trust_remote_code
        safe_valid = [r for r in valid if not r["trust_remote_code"]]
        if safe_valid:
            safe_scores = {r["model"]: scores[r["model"]] for r in safe_valid}
            safe_winner = max(safe_scores, key=safe_scores.get)
            print(f"🔒 Best without trust_remote_code: {safe_winner}  (score={safe_scores[safe_winner]:.3f})")
    else:
        print("🔒 Winner does NOT require trust_remote_code — safe default ✓")

    return winner


def main():
    print("=" * 70)
    print("  CONTEXTIA EMBEDDING BENCHMARK — AUTORESEARCH LOOP")
    print("  Indexing full codebase, 50 queries, 3 runs per query")
    print("=" * 70)

    src_root = Path(__file__).parent.parent / "src" / "contextia_mcp"
    print(f"\nBuilding corpus from {src_root}...")
    corpus = build_full_corpus(src_root)
    print(f"Corpus: {len(corpus)} symbols from {len(set(Path(t.split('file: ')[1].split(chr(10))[0]) for _, t in corpus if 'file: ' in t))} files")

    models_to_test = ["bge-small-en", "jina-code", "nomic-embed", "codesearch-modernbert"]

    all_results = []
    for model_key in models_to_test:
        result = run_full_benchmark(model_key, corpus)
        all_results.append(result)

    winner = print_final_results(all_results)

    # Save results
    out = Path(__file__).parent / "benchmark_results_full.json"
    out.write_text(json.dumps(all_results, indent=2))
    print(f"\nResults saved to {out}")
    print(f"\nRECOMMENDATION: Set default model to '{winner}'")


if __name__ == "__main__":
    main()
