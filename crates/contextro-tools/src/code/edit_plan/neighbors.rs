use super::*;

pub(crate) fn is_edit_plan_stopword(token: &str) -> bool {
    matches!(
        token,
        "add"
            | "change"
            | "function"
            | "extract"
            | "file"
            | "goal"
            | "into"
            | "move"
            | "per"
            | "plan"
            | "refactor"
            | "rename"
            | "replace"
            | "result"
            | "separate"
            | "the"
            | "this"
            | "tool"
            | "update"
            | "using"
    )
}

pub(crate) fn edit_plan_goal_terms(goal: &str) -> Vec<String> {
    tokenize_codebase_map_text(goal)
        .into_iter()
        .filter(|token| !is_edit_plan_stopword(token))
        .collect()
}

pub(crate) fn edit_plan_symbol_name_tokens(node: &UniversalNode) -> HashSet<String> {
    let qualified_name = node
        .parent
        .as_ref()
        .map(|parent| format!("{parent}.{}", node.name))
        .unwrap_or_else(|| node.name.clone());
    [node.name.as_str(), qualified_name.as_str()]
        .into_iter()
        .flat_map(tokenize_codebase_map_text)
        .collect()
}

pub(crate) fn edit_plan_name_overlap(
    node: &UniversalNode,
    concept_tokens: &HashSet<String>,
) -> usize {
    let name_tokens = edit_plan_symbol_name_tokens(node);
    concept_tokens
        .iter()
        .filter(|term| {
            name_tokens.iter().any(|candidate| {
                candidate == *term
                    || candidate.contains(term.as_str())
                    || term.contains(candidate.as_str())
            })
        })
        .count()
}

pub(crate) fn add_edit_plan_symbol(
    affected_symbols: &mut Vec<Value>,
    seen_symbol_ids: &mut HashSet<String>,
    graph: &CodeGraph,
    node: &UniversalNode,
    codebase: Option<&str>,
    role: &str,
) {
    if !seen_symbol_ids.insert(node.id.clone()) {
        return;
    }
    let (callers, callees) = graph.get_node_degree(&node.id);
    affected_symbols.push(json!({
        "name": node.name,
        "file": strip_base(&node.location.file_path, codebase),
        "line": node.location.start_line,
        "callers": callers,
        "callees": callees,
        "role": role,
    }));
}

pub(crate) struct EditPlanOutputs<'a> {
    pub(crate) affected_symbols: &'a mut Vec<Value>,
    pub(crate) seen_symbol_ids: &'a mut HashSet<String>,
    pub(crate) target_files: &'a mut Vec<String>,
    pub(crate) risks: &'a mut Vec<String>,
}

pub(crate) fn add_edit_plan_neighbors(
    outputs: &mut EditPlanOutputs<'_>,
    graph: &CodeGraph,
    node: &UniversalNode,
    goal_terms: &HashSet<String>,
    codebase: Option<&str>,
) {
    let anchor_terms = edit_plan_symbol_name_tokens(node);
    let (anchor_callers, anchor_callees) = graph.get_node_degree(&node.id);
    let high_degree_anchor =
        anchor_callers >= 2 || anchor_callees >= 5 || anchor_callers + anchor_callees >= 6;
    let mut seen_neighbors = HashSet::new();
    let mut neighbor_candidates: Vec<UniversalNode> = graph
        .get_callers(&node.id)
        .into_iter()
        .chain(graph.get_callees(&node.id))
        .filter(|neighbor| seen_neighbors.insert(neighbor.id.clone()))
        .collect();
    neighbor_candidates.extend(graph.find_nodes_by_name("", false).into_iter().filter(
        |candidate| {
            candidate.location.file_path == node.location.file_path
                && candidate.location.start_line <= node.location.start_line
                && candidate.name.chars().any(|ch| ch.is_ascii_uppercase())
                && seen_neighbors.insert(candidate.id.clone())
        },
    ));
    let mut neighbors: Vec<(UniversalNode, f64)> = neighbor_candidates
        .into_iter()
        .filter_map(|neighbor| {
            if is_edit_plan_test_like_node(&neighbor) {
                return None;
            }

            let goal_overlap = codebase_map_symbol_concept_overlap(&neighbor, goal_terms) as f64;
            let anchor_overlap =
                codebase_map_symbol_concept_overlap(&neighbor, &anchor_terms) as f64;
            let goal_name_overlap = edit_plan_name_overlap(&neighbor, goal_terms) as f64;
            let anchor_name_overlap = edit_plan_name_overlap(&neighbor, &anchor_terms) as f64;
            let conceptual_overlap = goal_overlap + anchor_overlap;
            let name_overlap = goal_name_overlap + anchor_name_overlap;
            let same_file = neighbor.location.file_path == node.location.file_path;
            let state_bridge = is_edit_plan_state_or_config_bridge_symbol(&neighbor);
            let owner_like = same_file
                && neighbor.location.start_line <= node.location.start_line
                && neighbor.name.chars().any(|ch| ch.is_ascii_uppercase());
            let handler_like = is_edit_plan_handler_symbol(&neighbor);
            if high_degree_anchor && !state_bridge && name_overlap < 1.0 {
                return None;
            }
            if high_degree_anchor
                && !state_bridge
                && goal_name_overlap == 0.0
                && anchor_name_overlap == 0.0
            {
                return None;
            }
            if high_degree_anchor && same_file && handler_like && goal_name_overlap == 0.0 {
                return None;
            }
            if high_degree_anchor && same_file && !owner_like && !state_bridge && name_overlap < 2.0
            {
                return None;
            }
            let helper_penalty = if is_codebase_map_meta_helper_symbol(&neighbor.name) {
                0.95
            } else if is_codebase_map_generic_helper_symbol(&neighbor.name)
                && conceptual_overlap < 2.0
            {
                0.40
            } else {
                0.0
            };
            let handler_penalty = if handler_like && goal_name_overlap == 0.0 {
                0.40
            } else {
                0.0
            };
            let state_bridge_bonus =
                if state_bridge && (goal_overlap >= 1.0 || anchor_overlap >= 1.0) {
                    0.25
                } else {
                    0.0
                };

            let score = goal_overlap * 0.28
                + anchor_overlap * 0.24
                + goal_name_overlap * 0.22
                + anchor_name_overlap * 0.18
                + if same_file { 0.35 } else { 0.10 }
                + 0.18
                + state_bridge_bonus
                - helper_penalty
                - handler_penalty;

            let min_score = if high_degree_anchor { 0.90 } else { 0.45 };
            (score >= min_score).then_some((neighbor, score))
        })
        .collect();
    neighbors.sort_by(|a, b| {
        let a_same_file = a.0.location.file_path == node.location.file_path;
        let b_same_file = b.0.location.file_path == node.location.file_path;
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| b_same_file.cmp(&a_same_file))
            .then_with(|| a.0.location.start_line.cmp(&b.0.location.start_line))
    });

    for (neighbor, _) in neighbors
        .into_iter()
        .take(if high_degree_anchor { 2 } else { 3 })
    {
        let file = strip_base(&neighbor.location.file_path, codebase);
        if !outputs.target_files.contains(&file) {
            outputs.target_files.push(file.clone());
        }
        add_edit_plan_symbol(
            outputs.affected_symbols,
            outputs.seen_symbol_ids,
            graph,
            &neighbor,
            codebase,
            "neighbor",
        );

        let (callers, _) = graph.get_node_degree(&neighbor.id);
        if callers > 5 {
            outputs.risks.push(format!(
                "{} has {} callers — high blast radius",
                neighbor.name, callers
            ));
        }
    }
}
