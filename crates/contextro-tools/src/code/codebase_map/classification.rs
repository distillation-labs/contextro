use super::*;

pub(crate) fn codebase_map_local_meta_helper_penalty(
    node: &UniversalNode,
    targets_product_surface: bool,
) -> f64 {
    let symbol_name = node
        .name
        .rsplit("::")
        .next()
        .unwrap_or(&node.name)
        .rsplit('.')
        .next()
        .unwrap_or(&node.name)
        .to_ascii_lowercase();

    if is_codebase_map_meta_helper_symbol(&symbol_name) {
        if targets_product_surface {
            0.72
        } else {
            0.42
        }
    } else {
        0.0
    }
}

pub(crate) fn codebase_map_local_connectivity_bias(graph: &CodeGraph, node: &UniversalNode) -> f64 {
    let (callers, callees) = graph.get_node_degree(&node.id);
    let total_degree = callers + callees;
    let shared_flow_bonus = if callers > 0 && callees > 0 {
        0.08
    } else {
        0.0
    };

    total_degree.min(4) as f64 * 0.07 + shared_flow_bonus
}

pub(crate) fn codebase_map_query_targets_product_surface(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();
    if codebase_map_query_targets_engine_internals(&lowered) {
        return false;
    }

    lowered.contains("how does")
        || lowered.contains("how do")
        || [
            "mcp",
            "noise",
            "output",
            "persist",
            "persistence",
            "ranking",
            "response",
            "surface",
            "tool",
            "workflow",
            "work",
        ]
        .iter()
        .any(|token| lowered.contains(token))
}

pub(crate) fn codebase_map_query_targets_engine_internals(lowered_query: &str) -> bool {
    [
        "bm25",
        "cache",
        "cached",
        "caching",
        "embedding",
        "evict",
        "eviction",
        "expire",
        "hnsw",
        "expiry",
        "ttl",
        "invalidation",
        "invalidate",
        "ivf",
        "lancedb",
        "model2vec",
        "tantivy",
    ]
    .iter()
    .any(|token| lowered_query.contains(token))
}

pub(crate) fn codebase_map_surface_bias(node: &UniversalNode) -> f64 {
    if is_probable_codebase_map_product_surface_node(node) {
        0.45
    } else if is_probable_codebase_map_engine_internal_node(node) {
        -0.18
    } else {
        0.0
    }
}

pub(crate) fn is_probable_codebase_map_product_surface_node(node: &UniversalNode) -> bool {
    let path = node.location.file_path.to_ascii_lowercase();
    let symbol_name = node.name.to_ascii_lowercase();

    symbol_name.starts_with("handle_")
        || path.contains("/contextro-tools/")
        || path.contains("/contextro-server/")
        || path.contains("/tools/")
        || path.contains("/server/")
}

pub(crate) fn is_probable_codebase_map_engine_internal_node(node: &UniversalNode) -> bool {
    let path = node.location.file_path.to_ascii_lowercase();
    let symbol_name = node.name.to_ascii_lowercase();

    path.contains("/contextro-engines/")
        || path.contains("/engines/")
        || matches!(
            symbol_name.as_str(),
            "execute_search" | "fuse" | "adaptive_weights" | "make_result" | "search"
        )
}

pub(crate) fn required_codebase_map_matches(token_count: usize) -> usize {
    match token_count {
        0 => 0,
        1 => 1,
        2 => 1,
        3 => 2,
        _ => 2,
    }
}
