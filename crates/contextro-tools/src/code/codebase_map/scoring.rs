use super::*;

pub(crate) fn codebase_map_match_score(
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
) -> f64 {
    codebase_map_match_score_with_path(node, normalized_query, query_tokens, true)
}

pub(crate) fn codebase_map_symbol_match_score(
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
) -> f64 {
    codebase_map_match_score_with_path(node, normalized_query, query_tokens, false)
}

pub(crate) fn codebase_map_match_score_with_path(
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    include_file_path: bool,
) -> f64 {
    if normalized_query.is_empty() {
        return 1.0;
    }

    let exact_match = codebase_map_exact_query_match(node, normalized_query, include_file_path);
    let candidate_tokens = codebase_map_candidate_tokens_with_path(node, include_file_path);
    let matched_terms = query_tokens
        .iter()
        .filter(|term| {
            candidate_tokens.iter().any(|candidate| {
                candidate == *term
                    || candidate.contains(term.as_str())
                    || term.contains(candidate.as_str())
            })
        })
        .count();

    if !exact_match && matched_terms < required_codebase_map_matches(query_tokens.len()) {
        return 0.0;
    }

    let overlap = if query_tokens.is_empty() {
        0.0
    } else {
        matched_terms as f64 / query_tokens.len() as f64
    };
    let exact_bonus = if exact_match { 1.0 } else { 0.0 };
    let content_bonus = if !node.content.is_empty()
        && node.content.to_ascii_lowercase().contains(normalized_query)
    {
        0.2
    } else {
        0.0
    };

    exact_bonus + overlap + content_bonus
}

pub(crate) fn codebase_map_intra_file_relevance_score(
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
        - codebase_map_local_meta_helper_penalty(node, targets_product_surface)
}

pub(crate) fn codebase_map_expansion_score(
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    seed_file: &str,
    dominant_file: Option<&str>,
    targets_product_surface: bool,
) -> f64 {
    let base = codebase_map_match_score(node, normalized_query, query_tokens)
        + if targets_product_surface {
            codebase_map_surface_bias(node)
        } else {
            0.0
        };
    let same_seed_file = node.location.file_path == seed_file;
    let same_dominant_file = dominant_file
        .map(|file| node.location.file_path == file)
        .unwrap_or(false);
    let helper_penalty = if is_codebase_map_generic_helper_symbol(&node.name)
        && !same_seed_file
        && !same_dominant_file
    {
        0.15
    } else {
        0.0
    };

    base + codebase_map_subsystem_role_bias(node, targets_product_surface)
        + 0.20
        + if same_seed_file { 0.32 } else { 0.0 }
        + if same_dominant_file { 0.16 } else { 0.0 }
        - helper_penalty
}

pub(crate) fn codebase_map_same_file_score(
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    concept_tokens: &HashSet<String>,
    targets_product_surface: bool,
    connectivity_bias: f64,
) -> f64 {
    let base = codebase_map_intra_file_relevance_score(
        node,
        normalized_query,
        query_tokens,
        targets_product_surface,
    );
    let concept_overlap = codebase_map_symbol_concept_overlap(node, concept_tokens) as f64;
    let exact_name_bonus = if node.name.to_ascii_lowercase().contains(normalized_query) {
        0.2
    } else {
        0.0
    };
    base + connectivity_bias + concept_overlap.min(4.0) * 0.22 + 0.20 + exact_name_bonus
}

pub(crate) fn codebase_map_narrow_file_relevance_score(
    hits: &[CodebaseMapHit],
    graph: &CodeGraph,
    normalized_query: &str,
    query_tokens: &[String],
    targets_product_surface: bool,
) -> f64 {
    let mut scores: Vec<f64> = hits
        .iter()
        .filter_map(|hit| {
            graph.get_node(&hit.node_id).map(|node| {
                codebase_map_intra_file_relevance_score(
                    &node,
                    normalized_query,
                    query_tokens,
                    targets_product_surface,
                )
            })
        })
        .collect();
    if scores.is_empty() {
        return hits.iter().map(|hit| hit.score).fold(0.0_f64, f64::max)
            + hits
                .first()
                .map(|hit| codebase_map_file_owner_bonus(&hit.source_file, query_tokens))
                .unwrap_or(0.0);
    }

    scores.sort_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));
    scores.into_iter().take(2).sum::<f64>()
        + hits
            .first()
            .map(|hit| codebase_map_file_owner_bonus(&hit.source_file, query_tokens))
            .unwrap_or(0.0)
}

pub(crate) fn codebase_map_file_owner_bonus(file_path: &str, query_tokens: &[String]) -> f64 {
    if query_tokens.is_empty() {
        return 0.0;
    }

    let file_stem = Path::new(file_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");
    let stem_tokens = tokenize_codebase_map_text(file_stem);
    if stem_tokens.is_empty() {
        return 0.0;
    }

    let overlap = query_tokens
        .iter()
        .filter(|term| {
            stem_tokens.iter().any(|candidate| {
                candidate == *term
                    || candidate.contains(term.as_str())
                    || term.contains(candidate.as_str())
            })
        })
        .count() as f64;

    if overlap == 0.0 {
        0.0
    } else {
        0.12 + overlap.min(2.0) * 0.16
    }
}

pub(crate) fn should_keep_codebase_map_neighbor(
    node: &UniversalNode,
    query_terms: &HashSet<String>,
    dominant_file: Option<&str>,
    seed_file: &str,
) -> bool {
    let query_overlap = codebase_map_concept_overlap(node, query_terms);
    if is_codebase_map_meta_helper_symbol(&node.name) && query_overlap < 2 {
        return false;
    }
    if query_overlap > 0 {
        return true;
    }

    let same_seed_file = node.location.file_path == seed_file;
    let same_dominant_file = dominant_file
        .map(|file| node.location.file_path == file)
        .unwrap_or(false);

    !(same_seed_file || same_dominant_file) || !is_codebase_map_generic_helper_symbol(&node.name)
}

pub(crate) fn should_keep_same_file_codebase_map_candidate(
    node: &UniversalNode,
    query_terms: &HashSet<String>,
    dominant_concepts: &HashSet<String>,
) -> bool {
    let query_overlap = codebase_map_symbol_concept_overlap(node, query_terms);
    if is_codebase_map_meta_helper_symbol(&node.name) && query_overlap < 2 {
        return false;
    }
    if query_overlap > 0 {
        return true;
    }

    let dominant_overlap = codebase_map_symbol_concept_overlap(node, dominant_concepts);
    dominant_overlap >= 3 && !is_codebase_map_meta_helper_symbol(&node.name)
}

pub(crate) fn detect_dominant_codebase_map_file(
    hits: &[CodebaseMapHit],
    graph: &CodeGraph,
    normalized_query: &str,
    query_tokens: &[String],
    targets_product_surface: bool,
    narrow_explanatory_query: bool,
) -> Option<String> {
    #[derive(Clone, Copy, Default)]
    struct FileStats {
        hit_count: usize,
        total_score: f64,
        concept_overlap: usize,
        product_surface_hits: usize,
    }

    let mut file_scores: HashMap<&str, FileStats> = HashMap::new();
    let concept_terms: HashSet<String> = query_tokens.iter().cloned().collect();

    if narrow_explanatory_query {
        let mut per_file_hits: HashMap<&str, Vec<f64>> = HashMap::new();
        for hit in hits.iter().take(12) {
            let entry = file_scores.entry(hit.source_file.as_str()).or_default();
            entry.hit_count += 1;
            if let Some(node) = graph.get_node(&hit.node_id) {
                let symbol_score = codebase_map_intra_file_relevance_score(
                    &node,
                    normalized_query,
                    query_tokens,
                    targets_product_surface,
                );
                per_file_hits
                    .entry(hit.source_file.as_str())
                    .or_default()
                    .push(symbol_score.max(0.0));
                entry.concept_overlap += codebase_map_symbol_concept_overlap(&node, &concept_terms);
                if targets_product_surface && is_probable_codebase_map_product_surface_node(&node) {
                    entry.product_surface_hits += 1;
                }
            } else {
                per_file_hits
                    .entry(hit.source_file.as_str())
                    .or_default()
                    .push(hit.score.max(0.0));
            }
        }

        for (file, mut scores) in per_file_hits {
            scores.sort_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));
            if let Some(entry) = file_scores.get_mut(file) {
                entry.total_score = scores.into_iter().take(2).sum();
            }
        }
    } else {
        for hit in hits.iter().take(12) {
            let entry = file_scores.entry(hit.source_file.as_str()).or_default();
            entry.hit_count += 1;
            entry.total_score += hit.score;
            if let Some(node) = graph.get_node(&hit.node_id) {
                entry.concept_overlap += codebase_map_concept_overlap(&node, &concept_terms);
                if targets_product_surface && is_probable_codebase_map_product_surface_node(&node) {
                    entry.product_surface_hits += 1;
                }
            }
        }
    }

    let mut ranked_files: Vec<(&str, f64, FileStats)> = file_scores
        .iter()
        .map(|(file, stats)| {
            let weighted_score = if narrow_explanatory_query {
                stats.total_score + codebase_map_file_owner_bonus(file, query_tokens)
            } else {
                stats.total_score
                    + stats.hit_count as f64 * 0.22
                    + stats.concept_overlap.min(8) as f64 * 0.08
                    + if targets_product_surface {
                        stats.product_surface_hits as f64 * 0.10
                    } else {
                        0.0
                    }
            };
            (*file, weighted_score, *stats)
        })
        .collect();
    ranked_files.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                if narrow_explanatory_query {
                    b.2.concept_overlap.cmp(&a.2.concept_overlap)
                } else {
                    b.2.hit_count.cmp(&a.2.hit_count)
                }
            })
            .then_with(|| a.0.cmp(b.0))
    });

    match ranked_files.as_slice() {
        [(file, weighted_score, stats), (.., second_score, _)]
            if stats.hit_count >= 3 && *weighted_score >= *second_score + 0.40 =>
        {
            Some((*file).to_string())
        }
        [(file, _, stats)] if stats.hit_count >= 3 => Some((*file).to_string()),
        _ => None,
    }
}
