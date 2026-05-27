use super::*;

pub(crate) fn codebase_map_query_prefers_subsystem_closure(query: &str) -> bool {
    let trimmed = query.trim();
    !trimmed.is_empty()
        && trimmed.split_whitespace().count() >= 3
        && !codebase_map_query_targets_tests(query)
}

pub(crate) fn codebase_map_query_is_narrow_explanatory(
    query: &str,
    query_tokens: &[String],
) -> bool {
    let lowered = query.to_ascii_lowercase();
    let has_explanatory_prefix = lowered.contains("how does")
        || lowered.contains("how do")
        || lowered.contains("what does")
        || lowered.starts_with("explain ");

    has_explanatory_prefix && !query_tokens.is_empty() && query_tokens.len() <= 4
}

pub(crate) fn apply_dominant_file_focus(
    hits: &mut Vec<CodebaseMapHit>,
    graph: &CodeGraph,
    dominant_file: &str,
    dominant_concepts: &HashSet<String>,
    subsystem_ids: Option<&HashSet<String>>,
) {
    let dominant_scores: Vec<f64> = hits
        .iter()
        .filter(|hit| hit.source_file == dominant_file)
        .map(|hit| hit.score)
        .collect();
    let same_file_hits = dominant_scores.len();
    if same_file_hits < 4 {
        return;
    }

    let dominant_top_score = dominant_scores.iter().copied().fold(0.0_f64, f64::max);
    let dominant_floor = dominant_scores
        .iter()
        .copied()
        .reduce(f64::min)
        .unwrap_or(0.0);

    hits.retain(|hit| {
        if hit.source_file == dominant_file {
            if let Some(subsystem_ids) = subsystem_ids {
                if !subsystem_ids.is_empty() {
                    return subsystem_ids.contains(&hit.node_id);
                }
            }
            return true;
        }

        let Some(node) = graph.get_node(&hit.node_id) else {
            return false;
        };
        let concept_overlap = codebase_map_concept_overlap(&node, dominant_concepts);
        let near_dominant = hit.score >= dominant_top_score * 0.90;
        let solid_floor = hit.score >= dominant_floor + 0.12;

        concept_overlap >= 3 && near_dominant && solid_floor
    });
}

pub(crate) fn codebase_map_concept_overlap(
    node: &UniversalNode,
    concept_tokens: &HashSet<String>,
) -> usize {
    let candidate_tokens = codebase_map_candidate_tokens(node);
    concept_tokens
        .iter()
        .filter(|term| {
            candidate_tokens.iter().any(|candidate| {
                candidate == *term
                    || candidate.contains(term.as_str())
                    || term.contains(candidate.as_str())
            })
        })
        .count()
}

pub(crate) fn codebase_map_symbol_concept_overlap(
    node: &UniversalNode,
    concept_tokens: &HashSet<String>,
) -> usize {
    let candidate_tokens = codebase_map_symbol_candidate_tokens(node);
    concept_tokens
        .iter()
        .filter(|term| {
            candidate_tokens.iter().any(|candidate| {
                candidate == *term
                    || candidate.contains(term.as_str())
                    || term.contains(candidate.as_str())
            })
        })
        .count()
}

pub(crate) fn codebase_map_candidate_tokens(node: &UniversalNode) -> HashSet<String> {
    codebase_map_candidate_tokens_with_path(node, true)
}

pub(crate) fn codebase_map_symbol_candidate_tokens(node: &UniversalNode) -> HashSet<String> {
    codebase_map_candidate_tokens_with_path(node, false)
}

pub(crate) fn codebase_map_candidate_tokens_with_path(
    node: &UniversalNode,
    include_file_path: bool,
) -> HashSet<String> {
    let qualified_name = node
        .parent
        .as_ref()
        .map(|parent| format!("{parent}.{}", node.name))
        .unwrap_or_else(|| node.name.clone());
    let mut tokens: HashSet<String> = [
        node.name.as_str(),
        qualified_name.as_str(),
        node.content.as_str(),
        node.docstring.as_deref().unwrap_or(""),
    ]
    .into_iter()
    .flat_map(tokenize_codebase_map_text)
    .collect();
    if include_file_path {
        tokens.extend(tokenize_codebase_map_text(&node.location.file_path));
    }
    tokens
}

pub(crate) fn codebase_map_exact_query_match(
    node: &UniversalNode,
    normalized_query: &str,
    include_file_path: bool,
) -> bool {
    let qualified_name = node
        .parent
        .as_ref()
        .map(|parent| format!("{parent}.{}", node.name))
        .unwrap_or_else(|| node.name.clone());
    let mut fields = vec![
        node.name.as_str(),
        qualified_name.as_str(),
        node.content.as_str(),
        node.docstring.as_deref().unwrap_or(""),
    ];
    if include_file_path {
        fields.push(node.location.file_path.as_str());
    }
    fields
        .into_iter()
        .any(|field| field.to_ascii_lowercase().contains(normalized_query))
}

pub(crate) fn is_codebase_map_generic_helper_symbol(symbol_name: &str) -> bool {
    let symbol_name = symbol_name
        .rsplit("::")
        .next()
        .unwrap_or(symbol_name)
        .rsplit('.')
        .next()
        .unwrap_or(symbol_name)
        .to_ascii_lowercase();

    symbol_name.starts_with("resolve_")
        || symbol_name.starts_with("normalize_")
        || symbol_name.starts_with("tokenize_")
        || symbol_name.starts_with("collect_")
        || symbol_name.ends_with("_by_degree")
}

pub(crate) fn is_codebase_map_meta_helper_symbol(symbol_name: &str) -> bool {
    let symbol_name = symbol_name
        .rsplit("::")
        .next()
        .unwrap_or(symbol_name)
        .rsplit('.')
        .next()
        .unwrap_or(symbol_name)
        .to_ascii_lowercase();

    symbol_name.starts_with("query_targets_")
        || symbol_name.starts_with("confidence_")
        || symbol_name == "accumulate_result"
        || symbol_name == "fuse_results"
        || symbol_name == "rerank_result_limit"
}

pub(crate) fn codebase_map_subsystem_role_bias(
    node: &UniversalNode,
    targets_product_surface: bool,
) -> f64 {
    if !targets_product_surface {
        return 0.0;
    }

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
        -0.45
    } else if symbol_name.starts_with("handle_") {
        0.22
    } else if symbol_name.starts_with("rerank_")
        || symbol_name.contains("_rerank")
        || symbol_name.starts_with("drop_")
        || symbol_name.starts_with("filter_")
        || symbol_name.contains("_filter")
        || symbol_name.contains("classifier")
        || symbol_name.starts_with("classify_")
        || symbol_name.contains("guard")
    {
        0.18
    } else if (symbol_name.starts_with("is_")
        && (symbol_name.contains("query")
            || symbol_name.contains("lookup")
            || symbol_name.contains("symbol")))
        || (symbol_name.contains("match")
            && (symbol_name.contains("query")
                || symbol_name.contains("symbol")
                || symbol_name.contains("result")))
        || (symbol_name.ends_with("_limit")
            && (symbol_name.contains("candidate") || symbol_name.contains("result")))
    {
        0.16
    } else {
        0.0
    }
}

pub(crate) fn codebase_map_has_behavioral_role(node: &UniversalNode) -> bool {
    codebase_map_subsystem_role_bias(node, true) > 0.0
}

pub(crate) fn codebase_map_anchor_score(
    graph: &CodeGraph,
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    targets_product_surface: bool,
) -> f64 {
    codebase_map_symbol_match_score(node, normalized_query, query_tokens)
        + if targets_product_surface {
            codebase_map_surface_bias(node)
        } else {
            0.0
        }
        + codebase_map_subsystem_role_bias(node, targets_product_surface)
        + codebase_map_local_connectivity_bias(graph, node)
        - codebase_map_local_meta_helper_penalty(node, targets_product_surface)
        - if is_codebase_map_generic_helper_symbol(&node.name)
            && !codebase_map_has_behavioral_role(node)
        {
            0.18
        } else {
            0.0
        }
}

pub(crate) fn codebase_map_subsystem_closure_score(
    graph: &CodeGraph,
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    targets_product_surface: bool,
    distance: usize,
) -> f64 {
    let distance_bonus = match distance {
        0 => 0.40,
        1 => 0.24,
        2 => 0.12,
        _ => 0.0,
    };
    let role_bonus = if distance > 0 && codebase_map_has_behavioral_role(node) {
        0.08
    } else {
        0.0
    };

    codebase_map_anchor_score(
        graph,
        node,
        normalized_query,
        query_tokens,
        targets_product_surface,
    ) + distance_bonus
        + role_bonus
}

pub(crate) fn select_dominant_file_subsystem_anchors(
    hits: &[CodebaseMapHit],
    graph: &CodeGraph,
    dominant_file: &str,
    normalized_query: &str,
    query_tokens: &[String],
    targets_product_surface: bool,
) -> Vec<String> {
    let mut anchor_candidates: Vec<(String, f64, u32)> = hits
        .iter()
        .filter(|hit| hit.source_file == dominant_file)
        .filter_map(|hit| {
            let node = graph.get_node(&hit.node_id)?;
            if is_codebase_map_meta_helper_symbol(&node.name)
                || hit.is_test_like
                || is_codebase_map_generic_helper_symbol(&node.name)
                    && !codebase_map_has_behavioral_role(&node)
            {
                return None;
            }

            let score = codebase_map_anchor_score(
                graph,
                &node,
                normalized_query,
                query_tokens,
                targets_product_surface,
            );
            if score < 0.55 {
                return None;
            }

            Some((node.id.clone(), score, node.location.start_line))
        })
        .collect();

    anchor_candidates.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.2.cmp(&b.2))
    });
    anchor_candidates.dedup_by(|a, b| a.0 == b.0);
    anchor_candidates
        .into_iter()
        .take(3)
        .map(|(node_id, _, _)| node_id)
        .collect()
}
