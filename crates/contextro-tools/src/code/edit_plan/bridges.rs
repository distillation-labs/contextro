use super::*;

pub(crate) fn edit_plan_candidate_kind_rank(node: &UniversalNode) -> usize {
    if is_edit_plan_test_like_node(node) {
        4
    } else if is_codebase_map_meta_helper_symbol(&node.name) {
        3
    } else if is_codebase_map_generic_helper_symbol(&node.name) {
        2
    } else if node.name.chars().any(|ch| ch.is_ascii_uppercase()) {
        0
    } else {
        1
    }
}

pub(crate) fn is_edit_plan_symbol_like_candidate(candidate: &str) -> bool {
    candidate.contains('_')
        || candidate.contains(':')
        || candidate.chars().any(|ch| ch.is_ascii_uppercase())
}

pub(crate) fn is_edit_plan_test_like_node(node: &UniversalNode) -> bool {
    is_test_file(&node.location.file_path) || is_probable_codebase_map_test_symbol(&node.name)
}

pub(crate) fn is_edit_plan_handler_symbol(node: &UniversalNode) -> bool {
    let symbol_name = node
        .name
        .rsplit("::")
        .next()
        .unwrap_or(&node.name)
        .rsplit('.')
        .next()
        .unwrap_or(&node.name)
        .to_ascii_lowercase();

    symbol_name.starts_with("handle_")
}

pub(crate) fn is_edit_plan_owner_like_symbol(node: &UniversalNode) -> bool {
    node.name
        .rsplit("::")
        .next()
        .unwrap_or(&node.name)
        .rsplit('.')
        .next()
        .unwrap_or(&node.name)
        .chars()
        .any(|ch| ch.is_ascii_uppercase())
}

pub(crate) fn is_edit_plan_state_or_config_bridge_symbol(node: &UniversalNode) -> bool {
    let symbol_name = node
        .name
        .rsplit("::")
        .next()
        .unwrap_or(&node.name)
        .rsplit('.')
        .next()
        .unwrap_or(&node.name)
        .to_ascii_lowercase();

    symbol_name.contains("state")
        || symbol_name.contains("config")
        || symbol_name.contains("context")
        || symbol_name.contains("settings")
}

pub(crate) fn expand_edit_plan_bridge_symbols(
    primary_symbols: &[UniversalNode],
    goal: &str,
    graph: &CodeGraph,
    all_nodes: &[UniversalNode],
    limit: usize,
) -> Vec<UniversalNode> {
    if primary_symbols.is_empty() {
        return Vec::new();
    }

    let goal_terms = edit_plan_goal_terms(goal);
    let goal_term_set: HashSet<String> = goal_terms.iter().cloned().collect();
    let primary_ids: HashSet<String> = primary_symbols.iter().map(|node| node.id.clone()).collect();
    let primary_paths: HashSet<String> = primary_symbols
        .iter()
        .map(|node| node.location.file_path.clone())
        .collect();
    let cross_file_primary_scope = primary_paths.len() > 1;
    let mut scored: Vec<(UniversalNode, f64)> = primary_symbols
        .iter()
        .cloned()
        .map(|node| {
            let score = 3.5
                + score_edit_plan_bridge_node(
                    &node,
                    &goal_term_set,
                    &primary_paths,
                    cross_file_primary_scope,
                    false,
                );
            (node, score)
        })
        .collect();
    let mut seen: HashSet<String> = primary_ids;

    for primary in primary_symbols {
        let caller_ids: HashSet<String> = graph
            .get_callers(&primary.id)
            .into_iter()
            .map(|node| node.id)
            .collect();
        let callee_ids: HashSet<String> = graph
            .get_callees(&primary.id)
            .into_iter()
            .map(|node| node.id)
            .collect();
        let mut local_seen = HashSet::new();
        let mut neighbor_candidates: Vec<UniversalNode> = graph
            .get_callers(&primary.id)
            .into_iter()
            .chain(graph.get_callees(&primary.id))
            .filter(|node| local_seen.insert(node.id.clone()))
            .collect();
        neighbor_candidates.extend(
            all_nodes
                .iter()
                .filter(|node| node.location.file_path == primary.location.file_path)
                .cloned(),
        );

        let mut ranked_neighbors: Vec<(UniversalNode, f64)> = neighbor_candidates
            .into_iter()
            .filter_map(|node| {
                if seen.contains(&node.id) {
                    return None;
                }
                if is_edit_plan_test_like_node(&node) {
                    return None;
                }
                let state_bridge = is_edit_plan_state_or_config_bridge_symbol(&node);
                let goal_name_overlap = edit_plan_name_overlap(&node, &goal_term_set);
                let is_caller = caller_ids.contains(&node.id);
                let is_callee = callee_ids.contains(&node.id);
                let same_file = node.location.file_path == primary.location.file_path;
                let owner_like = same_file
                    && node.location.start_line <= primary.location.start_line
                    && is_edit_plan_owner_like_symbol(&node);
                if is_callee && goal_name_overlap == 0 && !state_bridge {
                    return None;
                }
                if same_file && !is_caller && goal_name_overlap == 0 && !state_bridge && !owner_like
                {
                    return None;
                }
                if cross_file_primary_scope
                    && same_file
                    && !state_bridge
                    && !owner_like
                    && (is_edit_plan_handler_symbol(&node) || goal_name_overlap < 2)
                {
                    return None;
                }
                let score = score_edit_plan_bridge_node(
                    &node,
                    &goal_term_set,
                    &primary_paths,
                    cross_file_primary_scope,
                    true,
                ) + if owner_like { 0.18 } else { 0.0 };
                (score >= 0.9).then_some((node, score))
            })
            .collect();
        ranked_neighbors.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.0.location.file_path.cmp(&b.0.location.file_path))
                .then_with(|| a.0.location.start_line.cmp(&b.0.location.start_line))
        });

        for (node, score) in ranked_neighbors.into_iter().take(2) {
            if seen.insert(node.id.clone()) {
                scored.push((node, 1.2 + score));
            }
        }
    }

    let primary_concepts: HashSet<String> = primary_symbols
        .iter()
        .flat_map(edit_plan_symbol_name_tokens)
        .chain(goal_terms.iter().cloned())
        .collect();

    let mut bridge_candidates: Vec<(UniversalNode, f64)> = all_nodes
        .iter()
        .filter_map(|node| {
            if seen.contains(&node.id) {
                return None;
            }
            if is_edit_plan_test_like_node(node) {
                return None;
            }
            let score = score_edit_plan_bridge_concept_match(
                node,
                &primary_concepts,
                &primary_paths,
                cross_file_primary_scope,
            );
            (score >= 1.35).then(|| (node.clone(), score))
        })
        .collect();
    bridge_candidates.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.location.file_path.cmp(&b.0.location.file_path))
            .then_with(|| a.0.location.start_line.cmp(&b.0.location.start_line))
    });

    for (node, score) in bridge_candidates.into_iter().take(2) {
        if seen.insert(node.id.clone()) {
            scored.push((node, score));
        }
    }

    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.location.file_path.cmp(&b.0.location.file_path))
            .then_with(|| a.0.location.start_line.cmp(&b.0.location.start_line))
    });
    scored.truncate(limit);
    scored.into_iter().map(|(node, _)| node).collect()
}

pub(crate) fn score_edit_plan_bridge_node(
    node: &UniversalNode,
    goal_terms: &HashSet<String>,
    primary_paths: &HashSet<String>,
    cross_file_primary_scope: bool,
    is_secondary: bool,
) -> f64 {
    let overlap = codebase_map_concept_overlap(node, goal_terms) as f64;
    let name_overlap = edit_plan_name_overlap(node, goal_terms) as f64;
    let same_primary_file = primary_paths.contains(&node.location.file_path);
    let symbol_like = is_edit_plan_symbol_like_candidate(&node.name);
    let state_bridge = is_edit_plan_state_or_config_bridge_symbol(node);
    let owner_like = is_edit_plan_owner_like_symbol(node);
    let path = node.location.file_path.to_ascii_lowercase();
    let helper_penalty = if is_codebase_map_meta_helper_symbol(&node.name) {
        0.95
    } else if is_codebase_map_generic_helper_symbol(&node.name) && overlap < 2.0 {
        0.45
    } else {
        0.0
    };
    let state_bridge_bonus = if state_bridge && overlap >= 1.0 {
        if cross_file_primary_scope && !same_primary_file {
            0.95 + if owner_like { 0.18 } else { 0.0 }
        } else {
            0.28
        }
    } else {
        0.0
    };
    let ungrounded_penalty = if name_overlap == 0.0 && !state_bridge {
        0.40
    } else {
        0.0
    };
    let handler_penalty = if is_edit_plan_handler_symbol(node) && name_overlap == 0.0 {
        0.55
    } else {
        0.0
    };
    let cross_file_same_path_penalty =
        if cross_file_primary_scope && same_primary_file && !state_bridge && !owner_like {
            if is_edit_plan_handler_symbol(node) {
                1.10
            } else if name_overlap < 1.0 {
                0.70
            } else {
                0.20
            }
        } else {
            0.0
        };

    overlap * 0.35
        + name_overlap * 0.22
        + if same_primary_file {
            if cross_file_primary_scope && !state_bridge && !owner_like {
                0.08
            } else {
                0.26
            }
        } else {
            0.0
        }
        + if symbol_like { 0.15 } else { 0.0 }
        + if path.contains("/server/") || path.contains("/tools/") || path.contains("/engines/") {
            0.08
        } else {
            0.0
        }
        + if is_secondary { 0.12 } else { 0.0 }
        + state_bridge_bonus
        - helper_penalty
        - ungrounded_penalty
        - handler_penalty
        - cross_file_same_path_penalty
}

pub(crate) fn score_edit_plan_bridge_concept_match(
    node: &UniversalNode,
    primary_concepts: &HashSet<String>,
    primary_paths: &HashSet<String>,
    cross_file_primary_scope: bool,
) -> f64 {
    let concept_overlap = codebase_map_concept_overlap(node, primary_concepts) as f64;
    let symbol_overlap = codebase_map_symbol_concept_overlap(node, primary_concepts) as f64;
    let name_overlap = edit_plan_name_overlap(node, primary_concepts) as f64;
    let same_path = primary_paths.contains(&node.location.file_path);
    let state_bridge = is_edit_plan_state_or_config_bridge_symbol(node);
    let owner_like = is_edit_plan_owner_like_symbol(node);
    if cross_file_primary_scope && same_path && !state_bridge && !owner_like && name_overlap < 2.0 {
        return 0.0;
    }
    if name_overlap == 0.0 && !state_bridge {
        return 0.0;
    }
    let helper_penalty = if is_codebase_map_meta_helper_symbol(&node.name) {
        1.1
    } else if is_codebase_map_generic_helper_symbol(&node.name) && name_overlap < 2.0 {
        0.55
    } else {
        0.0
    };
    let state_bridge_bonus = if state_bridge && (concept_overlap >= 2.0 || name_overlap >= 1.0) {
        if cross_file_primary_scope && !same_path {
            1.10 + if owner_like { 0.18 } else { 0.0 }
        } else {
            0.72
        }
    } else {
        0.0
    };
    let handler_penalty = if is_edit_plan_handler_symbol(node) && name_overlap < 1.0 {
        0.40
    } else {
        0.0
    };
    let cross_file_same_path_penalty =
        if cross_file_primary_scope && same_path && !state_bridge && !owner_like {
            if is_edit_plan_handler_symbol(node) {
                0.90
            } else if name_overlap < 2.0 {
                0.60
            } else {
                0.20
            }
        } else {
            0.0
        };

    concept_overlap * 0.30
        + symbol_overlap * 0.12
        + name_overlap * 0.28
        + if same_path {
            if cross_file_primary_scope && !state_bridge && !owner_like {
                0.04
            } else {
                0.16
            }
        } else {
            0.0
        }
        + if is_edit_plan_symbol_like_candidate(&node.name) {
            0.10
        } else {
            0.0
        }
        + state_bridge_bonus
        - helper_penalty
        - handler_penalty
        - cross_file_same_path_penalty
}
