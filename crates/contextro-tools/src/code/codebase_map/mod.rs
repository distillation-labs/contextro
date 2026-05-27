use super::*;

mod classification;
mod output;
mod scoring;
mod subsystem;
mod subsystem_closure;
mod tokens;

pub(crate) use classification::*;
pub(crate) use output::*;
pub(crate) use scoring::*;
pub(crate) use subsystem::*;
pub(crate) use subsystem_closure::*;
pub(crate) use tokens::*;

#[derive(Clone)]
pub(crate) struct CodebaseMapHit {
    node_id: String,
    file: String,
    source_file: String,
    score: f64,
    is_test_like: bool,
    symbol: Value,
}

/// Return a symbol-level map of the codebase grouped by file.
/// Accepts an optional `query` to filter by symbol name and an optional `path` prefix.
pub(crate) fn search_codebase_map(
    args: &Value,
    graph: &CodeGraph,
    codebase: Option<&str>,
) -> Value {
    let raw_query = args
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let normalized_query = raw_query.to_ascii_lowercase();
    let query_tokens = tokenize_codebase_map_text(raw_query);
    let query_term_set: HashSet<String> = query_tokens.iter().cloned().collect();
    let narrow_explanatory_query =
        codebase_map_query_is_narrow_explanatory(raw_query, &query_tokens);
    let targets_product_surface = codebase_map_query_targets_product_surface(raw_query);
    let prefers_subsystem_closure = codebase_map_query_prefers_subsystem_closure(raw_query);
    let path_filter = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let resolved_filter = if path_filter.is_empty() {
        None
    } else {
        match resolve_existing_path(path_filter, codebase) {
            Ok(path) => Some(path),
            Err(error) => return error,
        }
    };
    let filter_is_dir = resolved_filter
        .as_ref()
        .map(|path| path.is_dir())
        .unwrap_or(false);

    let all_nodes = graph.find_nodes_by_name("", false);
    let mut hits = Vec::new();
    for node in &all_nodes {
        if let Some(filter_path) = resolved_filter.as_ref() {
            if !path_matches(&node.location.file_path, filter_path, filter_is_dir) {
                continue;
            }
        }
        let mut score = codebase_map_match_score(node, &normalized_query, &query_tokens);
        if targets_product_surface {
            score += codebase_map_surface_bias(node);
        }
        if !normalized_query.is_empty() && score <= 0.0 {
            continue;
        }
        let rel = strip_base(&node.location.file_path, codebase);
        let (callers, callees) = graph.get_node_degree(&node.id);
        hits.push(CodebaseMapHit {
            node_id: node.id.clone(),
            is_test_like: is_test_file(&node.location.file_path)
                || is_probable_codebase_map_test_symbol(&node.name),
            file: rel,
            source_file: node.location.file_path.clone(),
            score,
            symbol: json!({
                "name": node.name,
                "type": node.node_type.to_string(),
                "line": node.location.start_line,
                "callers": callers,
                "callees": callees,
            }),
        });
    }

    let mut subsystem_dominant_file: Option<String> = None;

    if !normalized_query.is_empty() {
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.file.cmp(&b.file))
                .then_with(|| {
                    a.symbol["line"]
                        .as_u64()
                        .unwrap_or(0)
                        .cmp(&b.symbol["line"].as_u64().unwrap_or(0))
                })
        });

        let dominant_file = detect_dominant_codebase_map_file(
            &hits,
            graph,
            &normalized_query,
            &query_tokens,
            targets_product_surface,
            narrow_explanatory_query,
        );
        if prefers_subsystem_closure {
            subsystem_dominant_file = dominant_file.clone();
        }

        let mut seed_ids = Vec::new();
        if let Some(dominant_file) = dominant_file.as_ref() {
            let mut dominant_hits: Vec<&CodebaseMapHit> = hits
                .iter()
                .filter(|hit| &hit.source_file == dominant_file)
                .collect();
            dominant_hits.sort_by(|a, b| {
                let a_score = graph
                    .get_node(&a.node_id)
                    .map(|node| {
                        codebase_map_intra_file_relevance_score(
                            &node,
                            &normalized_query,
                            &query_tokens,
                            targets_product_surface,
                        ) + codebase_map_local_connectivity_bias(graph, &node)
                    })
                    .unwrap_or(a.score);
                let b_score = graph
                    .get_node(&b.node_id)
                    .map(|node| {
                        codebase_map_intra_file_relevance_score(
                            &node,
                            &normalized_query,
                            &query_tokens,
                            targets_product_surface,
                        ) + codebase_map_local_connectivity_bias(graph, &node)
                    })
                    .unwrap_or(b.score);
                b_score
                    .partial_cmp(&a_score)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| {
                        a.symbol["line"]
                            .as_u64()
                            .unwrap_or(0)
                            .cmp(&b.symbol["line"].as_u64().unwrap_or(0))
                    })
            });

            for hit in dominant_hits.into_iter().take(3) {
                seed_ids.push(hit.node_id.clone());
            }
        }
        for hit in &hits {
            if seed_ids.len() >= 3 {
                break;
            }
            if !seed_ids.iter().any(|id| id == &hit.node_id) {
                seed_ids.push(hit.node_id.clone());
            }
        }

        if let Some(dominant_file) = dominant_file.as_ref() {
            if prefers_subsystem_closure {
                let (subsystem_hits, _) = collect_dominant_file_subsystem_hits(
                    &hits,
                    graph,
                    dominant_file,
                    &normalized_query,
                    &query_tokens,
                    targets_product_surface,
                    codebase,
                );
                hits.extend(subsystem_hits);
            }
        }

        let mut expanded_hits = Vec::new();

        for seed_id in seed_ids {
            let Some(seed) = graph.get_node(&seed_id) else {
                continue;
            };

            let seed_file = seed.location.file_path.clone();
            let mut seen_neighbors = HashSet::new();
            let mut neighbors: Vec<UniversalNode> = graph
                .get_callers(&seed_id)
                .into_iter()
                .chain(graph.get_callees(&seed_id))
                .filter(|node| seen_neighbors.insert(node.id.clone()))
                .collect();
            neighbors.sort_by(|a, b| {
                let a_score = codebase_map_expansion_score(
                    a,
                    &normalized_query,
                    &query_tokens,
                    &seed_file,
                    dominant_file.as_deref(),
                    targets_product_surface,
                );
                let b_score = codebase_map_expansion_score(
                    b,
                    &normalized_query,
                    &query_tokens,
                    &seed_file,
                    dominant_file.as_deref(),
                    targets_product_surface,
                );
                b_score
                    .partial_cmp(&a_score)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| a.location.start_line.cmp(&b.location.start_line))
            });

            for node in neighbors.into_iter().take(4) {
                if !should_keep_codebase_map_neighbor(
                    &node,
                    &query_term_set,
                    dominant_file.as_deref(),
                    &seed_file,
                ) {
                    continue;
                }
                let score = codebase_map_expansion_score(
                    &node,
                    &normalized_query,
                    &query_tokens,
                    &seed_file,
                    dominant_file.as_deref(),
                    targets_product_surface,
                );
                let rel = strip_base(&node.location.file_path, codebase);
                let (callers, callees) = graph.get_node_degree(&node.id);
                expanded_hits.push(CodebaseMapHit {
                    node_id: node.id.clone(),
                    is_test_like: is_test_file(&node.location.file_path)
                        || is_probable_codebase_map_test_symbol(&node.name),
                    file: rel,
                    source_file: node.location.file_path.clone(),
                    score,
                    symbol: json!({
                        "name": node.name,
                        "type": node.node_type.to_string(),
                        "line": node.location.start_line,
                        "callers": callers,
                        "callees": callees,
                    }),
                });
            }
        }

        hits.extend(expanded_hits);

        if let Some(dominant_file) = dominant_file.as_ref() {
            if !prefers_subsystem_closure {
                let dominant_concepts: HashSet<String> = hits
                    .iter()
                    .filter(|hit| &hit.source_file == dominant_file)
                    .take(6)
                    .filter_map(|hit| graph.get_node(&hit.node_id))
                    .flat_map(|node| codebase_map_symbol_candidate_tokens(&node))
                    .chain(query_tokens.iter().cloned())
                    .collect();

                let mut same_file_candidates: Vec<(UniversalNode, f64)> = all_nodes
                    .iter()
                    .filter(|node| node.location.file_path == *dominant_file)
                    .filter(|node| {
                        should_keep_same_file_codebase_map_candidate(
                            node,
                            &query_term_set,
                            &dominant_concepts,
                        )
                    })
                    .map(|node| {
                        (
                            node.clone(),
                            codebase_map_same_file_score(
                                node,
                                &normalized_query,
                                &query_tokens,
                                &dominant_concepts,
                                targets_product_surface,
                                codebase_map_local_connectivity_bias(graph, node),
                            ),
                        )
                    })
                    .filter(|(_, score)| *score >= 0.50)
                    .collect();
                same_file_candidates.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| a.0.location.start_line.cmp(&b.0.location.start_line))
                });

                for (node, score) in same_file_candidates.into_iter().take(8) {
                    let rel = strip_base(&node.location.file_path, codebase);
                    let (callers, callees) = graph.get_node_degree(&node.id);
                    hits.push(CodebaseMapHit {
                        node_id: node.id.clone(),
                        is_test_like: is_test_file(&node.location.file_path)
                            || is_probable_codebase_map_test_symbol(&node.name),
                        file: rel,
                        source_file: node.location.file_path.clone(),
                        score,
                        symbol: json!({
                            "name": node.name,
                            "type": node.node_type.to_string(),
                            "line": node.location.start_line,
                            "callers": callers,
                            "callees": callees,
                        }),
                    });
                }
            }
        }
    }

    if !codebase_map_query_targets_tests(raw_query) && hits.iter().any(|hit| !hit.is_test_like) {
        hits.retain(|hit| !hit.is_test_like);
    }

    let mut deduped_hits: HashMap<String, CodebaseMapHit> = HashMap::new();
    for hit in hits {
        match deduped_hits.get_mut(&hit.node_id) {
            Some(existing) if existing.score >= hit.score => {}
            Some(existing) => *existing = hit,
            None => {
                deduped_hits.insert(hit.node_id.clone(), hit);
            }
        }
    }
    let mut hits: Vec<CodebaseMapHit> = deduped_hits.into_values().collect();

    if !normalized_query.is_empty() {
        let dominant_file = if prefers_subsystem_closure {
            subsystem_dominant_file.clone().or_else(|| {
                detect_dominant_codebase_map_file(
                    &hits,
                    graph,
                    &normalized_query,
                    &query_tokens,
                    targets_product_surface,
                    narrow_explanatory_query,
                )
            })
        } else {
            detect_dominant_codebase_map_file(
                &hits,
                graph,
                &normalized_query,
                &query_tokens,
                targets_product_surface,
                narrow_explanatory_query,
            )
        };

        if let Some(dominant_file) = dominant_file.as_ref() {
            let subsystem_nodes = if prefers_subsystem_closure {
                build_dominant_file_subsystem_nodes(
                    &hits,
                    graph,
                    dominant_file,
                    &normalized_query,
                    &query_tokens,
                    targets_product_surface,
                )
            } else {
                Vec::new()
            };
            let subsystem_ids: HashSet<String> = subsystem_nodes
                .iter()
                .map(|(node, _, _)| node.id.clone())
                .collect();
            let dominant_concepts: HashSet<String> =
                if prefers_subsystem_closure && !subsystem_nodes.is_empty() {
                    subsystem_nodes
                        .iter()
                        .flat_map(|(node, _, _)| codebase_map_symbol_candidate_tokens(node))
                        .chain(query_tokens.iter().cloned())
                        .collect()
                } else {
                    hits.iter()
                        .filter(|hit| &hit.source_file == dominant_file)
                        .take(8)
                        .filter_map(|hit| graph.get_node(&hit.node_id))
                        .flat_map(|node| codebase_map_symbol_candidate_tokens(&node))
                        .chain(query_tokens.iter().cloned())
                        .collect()
                };

            for hit in &mut hits {
                let Some(node) = graph.get_node(&hit.node_id) else {
                    continue;
                };
                let concept_overlap = if &hit.source_file == dominant_file {
                    codebase_map_symbol_concept_overlap(&node, &dominant_concepts) as f64
                } else {
                    codebase_map_concept_overlap(&node, &dominant_concepts) as f64
                };
                if &hit.source_file == dominant_file {
                    hit.score += 0.18 + concept_overlap.min(3.0) * 0.05;
                } else if concept_overlap == 0.0 {
                    hit.score = (hit.score - 0.25).max(0.0);
                } else if concept_overlap < 2.0 {
                    hit.score = (hit.score - 0.10).max(0.0);
                }
            }

            if prefers_subsystem_closure {
                apply_dominant_file_focus(
                    &mut hits,
                    graph,
                    dominant_file,
                    &dominant_concepts,
                    Some(&subsystem_ids),
                );
            }
        }
    }

    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| {
                a.symbol["line"]
                    .as_u64()
                    .unwrap_or(0)
                    .cmp(&b.symbol["line"].as_u64().unwrap_or(0))
            })
    });

    build_codebase_map_response(
        hits,
        graph,
        &normalized_query,
        &query_tokens,
        narrow_explanatory_query,
        targets_product_surface,
        raw_query,
        resolved_filter.as_deref(),
        codebase,
    )
}
