use super::*;

pub(crate) fn should_include_subsystem_closure_node(
    graph: &CodeGraph,
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    query_terms: &HashSet<String>,
    targets_product_surface: bool,
    distance: usize,
) -> bool {
    if is_codebase_map_meta_helper_symbol(&node.name)
        || is_probable_codebase_map_test_symbol(&node.name)
    {
        return false;
    }

    let score = codebase_map_subsystem_closure_score(
        graph,
        node,
        normalized_query,
        query_tokens,
        targets_product_surface,
        distance,
    );
    let query_overlap = codebase_map_symbol_concept_overlap(node, query_terms);
    let behavioral_role = codebase_map_has_behavioral_role(node);
    let generic_non_behavioral =
        is_codebase_map_generic_helper_symbol(&node.name) && !behavioral_role;
    if generic_non_behavioral && query_overlap == 0 {
        return false;
    }

    match distance {
        0 => score >= 0.55,
        1 => score >= 0.78 && (behavioral_role || query_overlap >= 1),
        2 => score >= 0.82 && behavioral_role && query_overlap >= 1,
        _ => false,
    }
}

pub(crate) fn should_expand_subsystem_closure_from(
    graph: &CodeGraph,
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    query_terms: &HashSet<String>,
    targets_product_surface: bool,
    distance: usize,
) -> bool {
    if distance >= 2 {
        return false;
    }

    let score = codebase_map_subsystem_closure_score(
        graph,
        node,
        normalized_query,
        query_tokens,
        targets_product_surface,
        distance,
    );
    let query_overlap = codebase_map_symbol_concept_overlap(node, query_terms);

    codebase_map_has_behavioral_role(node) || query_overlap >= 2 || score >= 1.15
}

pub(crate) fn build_dominant_file_subsystem_nodes(
    hits: &[CodebaseMapHit],
    graph: &CodeGraph,
    dominant_file: &str,
    normalized_query: &str,
    query_tokens: &[String],
    targets_product_surface: bool,
) -> Vec<(UniversalNode, usize, f64)> {
    let anchor_ids = select_dominant_file_subsystem_anchors(
        hits,
        graph,
        dominant_file,
        normalized_query,
        query_tokens,
        targets_product_surface,
    );
    if anchor_ids.is_empty() {
        return Vec::new();
    }

    let query_terms: HashSet<String> = query_tokens.iter().cloned().collect();
    let mut queue: VecDeque<(String, usize)> = anchor_ids
        .iter()
        .cloned()
        .map(|node_id| (node_id, 0))
        .collect();
    let mut best_nodes: HashMap<String, (usize, f64)> = HashMap::new();

    for anchor_id in &anchor_ids {
        let Some(anchor) = graph.get_node(anchor_id) else {
            continue;
        };
        let score = codebase_map_subsystem_closure_score(
            graph,
            &anchor,
            normalized_query,
            query_tokens,
            targets_product_surface,
            0,
        );
        best_nodes.insert(anchor.id.clone(), (0, score));
    }

    while let Some((node_id, distance)) = queue.pop_front() {
        let Some(node) = graph.get_node(&node_id) else {
            continue;
        };
        if node.location.file_path != dominant_file || distance >= 2 {
            continue;
        }

        let mut seen_neighbors = HashSet::new();
        let neighbors: Vec<UniversalNode> = graph
            .get_callers(&node_id)
            .into_iter()
            .chain(graph.get_callees(&node_id))
            .filter(|neighbor| {
                neighbor.location.file_path == dominant_file
                    && seen_neighbors.insert(neighbor.id.clone())
            })
            .collect();

        for neighbor in neighbors {
            let next_distance = distance + 1;
            if !should_include_subsystem_closure_node(
                graph,
                &neighbor,
                normalized_query,
                query_tokens,
                &query_terms,
                targets_product_surface,
                next_distance,
            ) {
                continue;
            }

            let score = codebase_map_subsystem_closure_score(
                graph,
                &neighbor,
                normalized_query,
                query_tokens,
                targets_product_surface,
                next_distance,
            );

            let should_update = match best_nodes.get(&neighbor.id) {
                Some((best_distance, best_score)) => {
                    next_distance < *best_distance
                        || (next_distance == *best_distance && score > *best_score)
                }
                None => true,
            };

            if should_update {
                best_nodes.insert(neighbor.id.clone(), (next_distance, score));
            }

            if should_expand_subsystem_closure_from(
                graph,
                &neighbor,
                normalized_query,
                query_tokens,
                &query_terms,
                targets_product_surface,
                next_distance,
            ) {
                queue.push_back((neighbor.id.clone(), next_distance));
            }
        }
    }

    let mut subsystem_nodes: Vec<(UniversalNode, usize, f64)> = best_nodes
        .into_iter()
        .filter_map(|(node_id, (distance, score))| {
            graph.get_node(&node_id).map(|node| (node, distance, score))
        })
        .collect();
    subsystem_nodes.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.0.location.start_line.cmp(&b.0.location.start_line))
    });
    subsystem_nodes.truncate(8);
    subsystem_nodes
}

pub(crate) fn collect_dominant_file_subsystem_hits(
    hits: &[CodebaseMapHit],
    graph: &CodeGraph,
    dominant_file: &str,
    normalized_query: &str,
    query_tokens: &[String],
    targets_product_surface: bool,
    codebase: Option<&str>,
) -> (Vec<CodebaseMapHit>, HashSet<String>) {
    let subsystem_nodes = build_dominant_file_subsystem_nodes(
        hits,
        graph,
        dominant_file,
        normalized_query,
        query_tokens,
        targets_product_surface,
    );
    let subsystem_ids: HashSet<String> = subsystem_nodes
        .iter()
        .map(|(node, _, _)| node.id.clone())
        .collect();
    let subsystem_hits = subsystem_nodes
        .into_iter()
        .map(|(node, _, score)| {
            let rel = strip_base(&node.location.file_path, codebase);
            let (callers, callees) = graph.get_node_degree(&node.id);
            CodebaseMapHit {
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
            }
        })
        .collect();

    (subsystem_hits, subsystem_ids)
}
