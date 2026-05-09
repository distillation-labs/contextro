"""Reciprocal Rank Fusion and graph relevance scoring.

Combines results from vector search, BM25, and graph relevance
into a single ranked list using Reciprocal Rank Fusion (RRF).
"""

import logging
import math
import re
from typing import Any, Dict, List, Optional

from contextro_mcp.indexing.chunker import _generate_chunk_id

logger = logging.getLogger(__name__)


def graph_relevance_search(
    graph_engine,
    query: str,
    limit: int = 10,
) -> List[Dict[str, Any]]:
    """Score graph nodes by structural importance for a query.

    Tokenizes the query, finds matching nodes by name, and scores
    them by graph centrality (weighted in-degree + out-degree).

    Args:
        graph_engine: RustworkxCodeGraph instance.
        query: Search query text.
        limit: Max results to return.

    Returns:
        List of result dicts with id, symbol_name, filepath, score, etc.
    """
    # Tokenize query into words (alphanumeric, 2+ chars)
    tokens = [t.lower() for t in re.split(r"\W+", query) if len(t) >= 2]
    if not tokens:
        return []

    # Find matching nodes across all tokens
    seen_ids: set = set()
    candidates: list = []
    for token in tokens:
        matches = graph_engine.find_nodes_by_name(token, exact=False)
        for node in matches:
            if node.id not in seen_ids:
                seen_ids.add(node.id)
                candidates.append(node)

    if not candidates:
        return []

    # Score by graph centrality: in_degree * 2 + out_degree
    # Exclude MCP server wrapper functions — they have artificially high centrality
    # because they call many codebase functions, but they're not part of the codebase.
    scored = []
    for node in candidates:
        # Skip nodes from the MCP server file itself
        if node.location.file_path.endswith("server.py"):
            continue
        in_deg, out_deg = graph_engine.get_node_degree(node.id)
        raw_score = in_deg * 2 + out_deg
        scored.append((node, raw_score))

    if not scored:
        return []

    # Normalize scores to [0, 1]
    max_score = max(s for _, s in scored) or 1
    scored.sort(key=lambda x: x[1], reverse=True)

    results = []
    for node, raw_score in scored[:limit]:
        # Map graph node to chunk ID for fusion deduplication
        chunk_id = _generate_chunk_id(node.location.file_path, node.name, node.location.start_line)
        results.append(
            {
                "id": chunk_id,
                "filepath": node.location.file_path,
                "symbol_name": node.name,
                "symbol_type": node.node_type.value,
                "language": node.language,
                "line_start": node.location.start_line,
                "line_end": node.location.end_line,
                "score": raw_score / max_score,
            }
        )

    return results


class ReciprocalRankFusion:
    """Combine ranked lists from multiple engines using RRF.

    RRF formula: rrf_score(d) = sum(weight_i / (k + rank_i(d)))
    where k is a constant (default 60) and rank is 1-based.
    """

    def __init__(
        self,
        weights: Optional[Dict[str, float]] = None,
        k: int = 60,
    ):
        self.weights = weights or {"vector": 0.5, "bm25": 0.3, "graph": 0.2}
        self.k = k

    @staticmethod
    def _score_entropy(results: List[Dict[str, Any]]) -> float:
        """Shannon entropy of a retriever's score distribution (lower = more confident)."""
        scores = [r.get("score", 0.0) for r in results if r.get("score", 0.0) > 0]
        if not scores:
            return 1.0
        total = sum(scores)
        if total <= 0:
            return 1.0
        probs = [s / total for s in scores]
        return -sum(p * math.log(p + 1e-10) for p in probs)

    def _adaptive_weights(self, ranked_lists: Dict[str, List[Dict[str, Any]]]) -> Dict[str, float]:
        """Compute per-query weights: inverse entropy, normalized, bounded by base weights.

        Falls back to base weights when entropies are too similar (< 0.1 spread).
        Gives zero weight to retrievers with degenerate (all-equal) score distributions.
        """
        entropies = {
            engine: self._score_entropy(results)
            for engine, results in ranked_lists.items()
            if engine in self.weights
        }
        if not entropies:
            return self.weights

        # Detect degenerate retrievers: all scores equal → max entropy for that list size
        # A retriever with n results all at score 1.0 has entropy = log(n)
        degenerate: set[str] = set()
        for engine, results in ranked_lists.items():
            if engine not in self.weights:
                continue
            scores = [r.get("score", 0.0) for r in results if r.get("score", 0.0) > 0]
            if len(scores) >= 2:
                score_range = max(scores) - min(scores)
                if score_range < 1e-6:  # all scores identical → degenerate
                    degenerate.add(engine)

        # If vector is degenerate, zero it out and redistribute to BM25/graph
        if degenerate:
            adjusted = {e: (0.0 if e in degenerate else w) for e, w in self.weights.items()}
            total = sum(adjusted.values())
            if total > 0:
                return {e: w / total for e, w in adjusted.items()}

        # Only adapt when there's meaningful spread between retrievers
        entropy_values = list(entropies.values())
        if max(entropy_values) - min(entropy_values) < 0.1:
            return self.weights

        # Inverse entropy: lower entropy → higher confidence → higher weight
        inv = {e: 1.0 / (v + 1e-6) for e, v in entropies.items()}
        total_inv = sum(inv.values())
        # Blend 50/50 with base weights to avoid over-correction
        adapted: Dict[str, float] = {}
        for engine, base_w in self.weights.items():
            entropy_w = inv.get(engine, 0.0) / total_inv if total_inv > 0 else base_w
            adapted[engine] = 0.5 * base_w + 0.5 * entropy_w
        # Normalize
        total = sum(adapted.values()) or 1.0
        return {e: w / total for e, w in adapted.items()}

    def fuse(self, ranked_lists: Dict[str, List[Dict[str, Any]]]) -> List[Dict[str, Any]]:
        """Fuse multiple ranked lists into one using RRF.

        Weights are adapted per-query using inverse entropy of each retriever's
        score distribution: a retriever with concentrated scores (low entropy)
        is more confident and gets a higher weight.
        """
        # Compute entropy-adaptive weights
        effective_weights = self._adaptive_weights(ranked_lists)
        # Accumulate RRF scores per chunk ID
        scores: Dict[str, float] = {}
        # Track best metadata per chunk (from highest-weight engine)
        metadata: Dict[str, Dict[str, Any]] = {}
        sources: Dict[str, List[str]] = {}

        # Process engines in weight order (highest first) so metadata
        # from highest-weight engine takes priority
        sorted_engines = sorted(
            effective_weights.keys(),
            key=lambda e: effective_weights.get(e, 0),
            reverse=True,
        )

        for engine_name in sorted_engines:
            if engine_name not in ranked_lists:
                continue

            results = ranked_lists[engine_name]
            weight = effective_weights.get(engine_name, 0.0)
            if weight <= 0:
                continue

            for rank, result in enumerate(results, start=1):
                doc_id = result.get("id")
                if not doc_id:
                    continue

                rrf_contribution = weight / (self.k + rank)
                scores[doc_id] = scores.get(doc_id, 0.0) + rrf_contribution

                # First engine to provide metadata wins (highest weight)
                if doc_id not in metadata:
                    metadata[doc_id] = {k: v for k, v in result.items() if k != "score"}

                if doc_id not in sources:
                    sources[doc_id] = []
                sources[doc_id].append(engine_name)

        # Build fused results sorted by RRF score
        fused = []
        for doc_id in sorted(scores.keys(), key=lambda d: scores[d], reverse=True):
            entry = metadata.get(doc_id, {"id": doc_id})
            entry["rrf_score"] = scores[doc_id]
            entry["score"] = scores[doc_id]
            entry["_fusion_sources"] = sources.get(doc_id, [])
            fused.append(entry)

        # Normalize scores to [0, 1] so they're meaningful to consumers
        if fused:
            max_score = fused[0]["score"]  # already sorted descending
            if max_score > 0:
                for entry in fused:
                    entry["score"] = round(entry["score"] / max_score, 4)

        return fused
