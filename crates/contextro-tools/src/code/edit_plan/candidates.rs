use super::*;

pub(crate) fn resolve_edit_plan_primary_symbols(
    symbol_name: Option<&str>,
    goal: &str,
    graph: &CodeGraph,
) -> Vec<UniversalNode> {
    if let Some(symbol_name) = symbol_name.filter(|name| !name.is_empty()) {
        let matches = graph.find_nodes_by_name(symbol_name, false);
        if !matches.is_empty() {
            return matches.into_iter().take(3).collect();
        }
    }

    infer_edit_plan_symbols_from_goal(goal, graph)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditPlanCandidateMatchTier {
    ExactName,
    FuzzyName,
    Content,
}

pub(crate) struct EditPlanCandidateMatch {
    node: UniversalNode,
    score: f64,
    tier: EditPlanCandidateMatchTier,
}

pub(crate) fn infer_edit_plan_symbols_from_goal(
    goal: &str,
    graph: &CodeGraph,
) -> Vec<UniversalNode> {
    let all_nodes = graph.find_nodes_by_name("", false);
    let all_candidates = extract_goal_symbol_candidates(goal);
    let anchor_candidates = rank_edit_plan_goal_candidates(goal, &all_candidates);
    let mut exact_ranked_matches: HashMap<String, (UniversalNode, f64)> = HashMap::new();
    let mut fuzzy_ranked_matches: HashMap<String, (UniversalNode, f64)> = HashMap::new();
    let mut content_ranked_matches: HashMap<String, (UniversalNode, f64)> = HashMap::new();

    for (candidate, candidate_score) in anchor_candidates {
        let candidate_matches = resolve_edit_plan_candidate_matches(&candidate, graph, &all_nodes);
        for candidate_match in candidate_matches {
            let score = candidate_score + candidate_match.score;
            let bucket = match candidate_match.tier {
                EditPlanCandidateMatchTier::ExactName => &mut exact_ranked_matches,
                EditPlanCandidateMatchTier::FuzzyName => &mut fuzzy_ranked_matches,
                EditPlanCandidateMatchTier::Content => &mut content_ranked_matches,
            };
            match bucket.get_mut(&candidate_match.node.id) {
                Some(existing) if existing.1 >= score => {}
                Some(existing) => *existing = (candidate_match.node, score),
                None => {
                    bucket.insert(
                        candidate_match.node.id.clone(),
                        (candidate_match.node, score),
                    );
                }
            }
        }
    }

    let mut ranked_matches: Vec<(UniversalNode, f64)> = if !exact_ranked_matches.is_empty() {
        exact_ranked_matches.into_values().collect()
    } else if !fuzzy_ranked_matches.is_empty() {
        fuzzy_ranked_matches.into_values().collect()
    } else {
        content_ranked_matches.into_values().collect()
    };

    ranked_matches.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.location.file_path.cmp(&b.0.location.file_path))
            .then_with(|| a.0.location.start_line.cmp(&b.0.location.start_line))
    });

    ranked_matches
        .into_iter()
        .take(3)
        .map(|(node, _)| node)
        .collect()
}

pub(crate) fn extract_goal_symbol_candidates(goal: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for token in goal.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')) {
        let token = token.trim_matches(':').trim_matches('_');
        if token.len() < 3 {
            continue;
        }
        let lower = token.to_ascii_lowercase();
        if is_edit_plan_stopword(&lower) {
            continue;
        }
        if seen.insert(lower) {
            candidates.push(token.to_string());
        }
    }

    candidates
}

pub(crate) fn rank_edit_plan_goal_candidates(
    goal: &str,
    candidates: &[String],
) -> Vec<(String, f64)> {
    let normalized_goal = goal.to_ascii_lowercase();
    let goal_tokens = tokenize_codebase_map_text(goal);
    let goal_terms: HashSet<String> = goal_tokens.iter().cloned().collect();
    let mut ranked = Vec::new();

    for (index, candidate) in candidates.iter().enumerate() {
        let candidate_lower = candidate.to_ascii_lowercase();
        let candidate_tokens = tokenize_codebase_map_text(candidate);
        let exact_phrase = normalized_goal.contains(candidate_lower.as_str());
        let symbol_like = is_edit_plan_symbol_like_candidate(candidate);
        let overlap = candidate_tokens
            .iter()
            .filter(|token| goal_terms.contains(*token))
            .count() as f64;

        let mut score = overlap * 0.25;
        if exact_phrase {
            score += 2.0;
        }
        if symbol_like {
            score += 1.3;
        }
        if candidate.chars().any(|ch| ch.is_ascii_uppercase()) {
            score += 0.45;
        }
        if candidate.contains('_') || candidate.contains(':') {
            score += 0.35;
        }
        if candidate.len() >= 8 {
            score += 0.15;
        }

        ranked.push((candidate.clone(), score - index as f64 * 0.03));
    }

    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| b.0.len().cmp(&a.0.len()))
            .then_with(|| a.0.cmp(&b.0))
    });
    ranked
}

pub(crate) fn resolve_edit_plan_candidate_matches(
    candidate: &str,
    graph: &CodeGraph,
    all_nodes: &[UniversalNode],
) -> Vec<EditPlanCandidateMatch> {
    let candidate_lower = candidate.to_ascii_lowercase();
    let candidate_tokens = tokenize_codebase_map_text(candidate);
    let mut matches: Vec<EditPlanCandidateMatch> = graph
        .find_nodes_by_name(candidate, true)
        .into_iter()
        .map(|node| {
            let score = score_edit_plan_candidate_match(&candidate_lower, &candidate_tokens, &node);
            EditPlanCandidateMatch {
                node,
                score: 3.0 + score,
                tier: EditPlanCandidateMatchTier::ExactName,
            }
        })
        .collect();

    if matches.is_empty() {
        matches = graph
            .find_nodes_by_name(candidate, false)
            .into_iter()
            .map(|node| {
                let score =
                    score_edit_plan_candidate_match(&candidate_lower, &candidate_tokens, &node);
                EditPlanCandidateMatch {
                    node,
                    score: 2.0 + score,
                    tier: EditPlanCandidateMatchTier::FuzzyName,
                }
            })
            .collect();
    }

    if matches.is_empty() {
        matches = all_nodes
            .iter()
            .filter_map(|node| {
                let score =
                    score_edit_plan_candidate_match(&candidate_lower, &candidate_tokens, node);
                (score >= 1.0).then(|| EditPlanCandidateMatch {
                    node: node.clone(),
                    score,
                    tier: EditPlanCandidateMatchTier::Content,
                })
            })
            .collect();
    }

    matches.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                edit_plan_candidate_kind_rank(&a.node).cmp(&edit_plan_candidate_kind_rank(&b.node))
            })
            .then_with(|| a.node.location.start_line.cmp(&b.node.location.start_line))
    });
    matches.truncate(4);
    matches
}

pub(crate) fn score_edit_plan_candidate_match(
    candidate_lower: &str,
    candidate_tokens: &[String],
    node: &UniversalNode,
) -> f64 {
    let node_name = node.name.to_ascii_lowercase();
    let qualified_name = node
        .parent
        .as_ref()
        .map(|parent| format!("{parent}.{}", node.name))
        .unwrap_or_else(|| node.name.clone())
        .to_ascii_lowercase();
    let content = node.content.to_ascii_lowercase();
    let docstring = node.docstring.as_deref().unwrap_or("").to_ascii_lowercase();
    let file_path = node.location.file_path.to_ascii_lowercase();

    let name_match_score = if node_name == candidate_lower || qualified_name == candidate_lower {
        3.0
    } else if node_name.contains(candidate_lower) || qualified_name.contains(candidate_lower) {
        2.2
    } else if candidate_lower.contains(node_name.as_str()) {
        1.4
    } else {
        0.0
    };
    let mut score = name_match_score;

    if content.contains(candidate_lower) {
        score += if node_name.contains(candidate_lower) || qualified_name.contains(candidate_lower)
        {
            0.75
        } else {
            0.25
        };
    }
    if docstring.contains(candidate_lower) {
        score += 0.5;
    }
    if file_path.contains(candidate_lower) {
        score += if node_name.contains(candidate_lower) || qualified_name.contains(candidate_lower)
        {
            0.2
        } else {
            0.05
        };
    }

    let token_overlap = candidate_tokens
        .iter()
        .filter(|token| {
            node_name.contains(token.as_str())
                || qualified_name.contains(token.as_str())
                || content.contains(token.as_str())
                || docstring.contains(token.as_str())
                || file_path.contains(token.as_str())
        })
        .count() as f64;
    let name_token_overlap = candidate_tokens
        .iter()
        .filter(|token| {
            node_name.contains(token.as_str()) || qualified_name.contains(token.as_str())
        })
        .count() as f64;
    let helper_penalty = if is_codebase_map_meta_helper_symbol(&node.name) {
        1.4
    } else if is_codebase_map_generic_helper_symbol(&node.name) {
        0.75
    } else {
        0.0
    };
    let test_penalty = if is_edit_plan_test_like_node(node)
        && !candidate_lower.contains("test")
        && !candidate_lower.contains("spec")
    {
        if name_match_score >= 2.2 {
            1.6
        } else {
            2.6
        }
    } else {
        0.0
    };
    let weak_content_only_penalty =
        if name_token_overlap == 0.0 && token_overlap > 0.0 && name_match_score == 0.0 {
            0.75
        } else {
            0.0
        };

    score + name_token_overlap * 0.35 + token_overlap * 0.12
        - helper_penalty
        - test_penalty
        - weak_content_only_penalty
}
