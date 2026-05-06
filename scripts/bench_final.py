"""Final comprehensive benchmark of all Contextia MCP tools."""
import json
import os
import sys
import time

sys.path.insert(0, "src")
os.environ["CTX_STORAGE_DIR"] = "/tmp/ctx_final_bench"

from pathlib import Path

from contextia_mcp.config import get_settings, reset_settings
from contextia_mcp.engines.fusion import ReciprocalRankFusion, graph_relevance_search
from contextia_mcp.git.commit_indexer import CommitHistoryIndexer, extract_commits
from contextia_mcp.indexing.embedding_service import EMBEDDING_MODELS, get_embedding_service
from contextia_mcp.indexing.pipeline import IndexingPipeline
from contextia_mcp.state import get_state, reset_state

reset_settings()
reset_state()
settings = get_settings()
repo = Path(__file__).resolve().parents[1]

print("=" * 60)
print("CONTEXTIA MCP — FINAL BENCHMARK")
print("=" * 60)

# Index
pipeline = IndexingPipeline(settings)
t0 = time.time()
result = pipeline.index(repo)
idx_time = time.time() - t0

state = get_state()
state.codebase_path = repo
state.vector_engine = pipeline.vector_engine
state.bm25_engine = pipeline.bm25_engine
state.graph_engine = pipeline.graph_engine
graph = state.graph_engine

print(f"\nIndex: {result.total_files} files, {result.total_symbols} symbols, "
      f"{result.total_chunks} chunks in {idx_time:.2f}s")

# Commits
embedding_svc = get_embedding_service(settings.embedding_model)
dims = EMBEDDING_MODELS.get(settings.embedding_model, {}).get("dimensions", 256)
commit_idx = CommitHistoryIndexer(embedding_service=embedding_svc, vector_dims=dims)
cr = commit_idx.index_commits(str(repo), str(settings.lancedb_path), limit=50, include_diffs=True)
print(f"Commits: {cr['total_commits']} indexed")

# Benchmark
log = []


def bench(name, fn):
    t0 = time.time()
    try:
        r = fn()
        ms = (time.time() - t0) * 1000
        out = json.dumps(r) if isinstance(r, (dict, list)) else str(r)
        log.append({"tool": name, "ms": round(ms), "bytes": len(out), "ok": True})
        return r
    except Exception as e:
        ms = (time.time() - t0) * 1000
        log.append({"tool": name, "ms": round(ms), "bytes": 0, "ok": False, "err": str(e)})
        return None


print("\n--- SEARCH ---")


def t_search_narrow():
    v = state.vector_engine.search("embedding batch processing", limit=10)
    b = state.bm25_engine.search("embedding batch processing", limit=10)
    g = graph_relevance_search(graph, "embedding batch processing", limit=10)
    f = ReciprocalRankFusion().fuse({"vector": v, "bm25": b, "graph": g})
    return {"total": len(f), "top": f[0].get("symbol_name", "?")[:50] if f else "?"}


bench("search(narrow)", t_search_narrow)


def t_search_broad():
    v = state.vector_engine.search("how does indexing work end to end", limit=10)
    b = state.bm25_engine.search("how does indexing work end to end", limit=10)
    f = ReciprocalRankFusion().fuse({"vector": v, "bm25": b})
    return {"total": len(f), "top": f[0].get("symbol_name", "?")[:50] if f else "?"}


bench("search(broad)", t_search_broad)


def t_search_bm25():
    return {"results": len(state.bm25_engine.search("IndexingPipeline", limit=5))}


bench("search(bm25)", t_search_bm25)

print("\n--- SYMBOLS ---")


def t_find_symbol():
    m = graph.find_nodes_by_name("IndexingPipeline", exact=True)
    if not m:
        return {"error": "not found"}
    n = m[0]
    c = graph.get_callers(n.id)
    return {"name": n.name, "file": str(Path(n.location.file_path).relative_to(repo)),
            "line": n.location.start_line, "callers": len(c),
            "top_callers": [x.name for x in c[:5]]}


bench("find_symbol(exact)", t_find_symbol)


def t_find_symbol_fuzzy():
    m = graph.find_nodes_by_name("embed", exact=False)
    return {"total": len(m), "shown": min(20, len(m)), "names": [x.name for x in m[:20]]}


bench("find_symbol(fuzzy)", t_find_symbol_fuzzy)

print("\n--- GRAPH ---")


def t_callers():
    m = graph.find_nodes_by_name("IndexingPipeline", exact=True)
    if not m:
        return {"error": "not found"}
    c = graph.get_callers(m[0].id)
    return {"total": len(c), "callers": [x.name for x in c[:10]]}


bench("find_callers", t_callers)


def t_callees():
    m = graph.find_nodes_by_name("IndexingPipeline", exact=True)
    if not m:
        return {"error": "not found"}
    c = graph.get_callees(m[0].id)
    return {"total": len(c), "callees": [x.name for x in c[:10]]}


bench("find_callees", t_callees)


def t_impact():
    m = graph.find_nodes_by_name("IndexingPipeline", exact=True)
    if not m:
        return {"error": "not found"}
    imp = graph.get_transitive_callers(m[0].id, max_depth=10)
    return {"total": len(imp), "impacted": [x.name for x in imp[:10]]}


bench("impact", t_impact)

print("\n--- ORIENTATION ---")


def t_overview():
    stats = graph.get_statistics()
    dirs = {}
    for fp in graph._file_nodes:
        try:
            parts = Path(fp).relative_to(repo).parts
        except ValueError:
            continue
        top = parts[0] if parts else "."
        dirs[top] = dirs.get(top, 0) + 1
    return {"files": stats.get("total_files", 0), "symbols": stats.get("total_nodes", 0),
            "dirs": dict(sorted(dirs.items(), key=lambda x: x[1], reverse=True)[:10]),
            "langs": stats.get("nodes_by_language", {})}


bench("overview", t_overview)


def t_architecture():
    layers = {}
    for fp in graph._file_nodes:
        try:
            parts = Path(fp).relative_to(repo).parts
        except ValueError:
            continue
        layer = str(Path(parts[0]) / parts[1]) if len(parts) >= 3 else parts[0] if parts else "root"
        if layer not in layers:
            layers[layer] = {"files": 0}
        layers[layer]["files"] += 1
    top = dict(sorted(layers.items(), key=lambda x: x[1]["files"], reverse=True)[:10])
    return {"layers": top}


bench("architecture", t_architecture)

print("\n--- GIT ---")


def t_commit_history():
    commits = extract_commits(str(repo), limit=5)
    return [{"hash": c.short_hash, "msg": c.message[:80], "date": c.timestamp[:10]} for c in commits]


bench("commit_history(5)", t_commit_history)


def t_commit_search():
    return commit_idx.search_commits(str(settings.lancedb_path), "background indexing fix", limit=3)


bench("commit_search", t_commit_search)

print("\n--- MEMORY ---")


def t_remember():
    import uuid

    from contextia_mcp.core.models import Memory, MemoryType
    from contextia_mcp.memory.memory_store import MemoryStore
    store = MemoryStore(str(settings.lancedb_path), embedding_svc, vector_dims=dims)
    mem = Memory(
        id=str(uuid.uuid4())[:8],
        content="IndexingPipeline uses tree-sitter for parsing",
        memory_type=MemoryType.NOTE,
        project="contextia",
        tags=["arch"],
    )
    store.remember(mem)
    return {"stored": True, "count": store.count()}


bench("remember", t_remember)


def t_recall():
    from contextia_mcp.memory.memory_store import MemoryStore
    store = MemoryStore(str(settings.lancedb_path), embedding_svc, vector_dims=dims)
    mems = store.recall("tree-sitter parsing", limit=3)
    return [{"content": m.content[:80], "type": m.memory_type.value} for m in mems]


bench("recall", t_recall)

# Summary
print("\n" + "=" * 60)
print("RESULTS")
print("=" * 60)
passed = sum(1 for r in log if r["ok"])
failed = sum(1 for r in log if not r["ok"])
total_bytes = sum(r["bytes"] for r in log)
total_ms = sum(r["ms"] for r in log)

print(f"\n  Tools: {len(log)} | Passed: {passed} | Failed: {failed}")
print(f"  Total output: {total_bytes:,} bytes ({total_bytes // 4:,} tokens)")
print(f"  Total time: {total_ms}ms | Avg: {total_ms // max(len(log), 1)}ms/tool")
print(f"\n{'Tool':<25} {'Time':>8} {'Size':>10} {'Status':>8}")
print("-" * 55)
for r in log:
    status = "OK" if r["ok"] else "FAIL"
    print(f"{r['tool']:<25} {r['ms']:>6}ms {r['bytes']:>8}B {status:>8}")

if failed:
    print("\nFailed tools:")
    for r in log:
        if not r["ok"]:
            print(f"  {r['tool']}: {r.get('err', '?')}")
