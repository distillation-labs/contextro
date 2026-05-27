use super::codebase_map::*;
use super::*;

mod bridges;
mod candidates;
mod neighbors;

use bridges::*;
use candidates::*;
use neighbors::*;

pub(crate) fn edit_plan(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let goal = args.get("goal").and_then(|v| v.as_str()).unwrap_or("");
    if goal.is_empty() {
        return json!({"error": "Missing required parameter: goal"});
    }
    let file_path = args.get("file_path").and_then(|v| v.as_str());
    let pattern = args.get("pattern").and_then(|v| v.as_str());
    let symbol_name = args
        .get("symbol_name")
        .or_else(|| args.get("name"))
        .and_then(|v| v.as_str());

    let mut target_files: Vec<String> = Vec::new();
    let mut affected_symbols: Vec<Value> = Vec::new();
    let mut risks: Vec<String> = Vec::new();
    let mut seen_symbol_ids = HashSet::new();
    let goal_term_set: HashSet<String> = edit_plan_goal_terms(goal).into_iter().collect();

    let primary_symbols = resolve_edit_plan_primary_symbols(symbol_name, goal, graph);
    let bridge_symbols = expand_edit_plan_bridge_symbols(
        &primary_symbols,
        goal,
        graph,
        &graph.find_nodes_by_name("", false),
        6,
    );

    for node in primary_symbols {
        let file = strip_base(&node.location.file_path, codebase);
        if !target_files.contains(&file) {
            target_files.push(file);
        }
        add_edit_plan_symbol(
            &mut affected_symbols,
            &mut seen_symbol_ids,
            graph,
            &node,
            codebase,
            "primary",
        );
        {
            let mut outputs = EditPlanOutputs {
                affected_symbols: &mut affected_symbols,
                seen_symbol_ids: &mut seen_symbol_ids,
                target_files: &mut target_files,
                risks: &mut risks,
            };
            add_edit_plan_neighbors(&mut outputs, graph, &node, &goal_term_set, codebase);
        }

        let (callers, _) = graph.get_node_degree(&node.id);
        if callers > 5 {
            risks.push(format!(
                "{} has {} callers — high blast radius",
                node.name, callers
            ));
        }
    }

    for node in bridge_symbols {
        let file = strip_base(&node.location.file_path, codebase);
        if !target_files.contains(&file) {
            target_files.push(file);
        }
        add_edit_plan_symbol(
            &mut affected_symbols,
            &mut seen_symbol_ids,
            graph,
            &node,
            codebase,
            "bridge",
        );
    }

    if let Some(fp) = file_path {
        let resolved = resolve_existing_path(fp, codebase)
            .ok()
            .map(|path| strip_base(&path.to_string_lossy(), codebase))
            .unwrap_or_else(|| fp.to_string());
        if !target_files.contains(&resolved) {
            target_files.push(resolved);
        }
    }

    target_files.sort();
    target_files.dedup();
    risks.sort();
    risks.dedup();

    // Find related tests
    let related_tests: Vec<String> = target_files
        .iter()
        .filter_map(|f| {
            let stem = Path::new(f).file_stem()?.to_string_lossy().to_string();
            let test_name = format!("test_{}", stem);
            let matches = graph.find_nodes_by_name(&test_name, false);
            if matches.is_empty() || matches.iter().all(|node| node.name != test_name) {
                None
            } else {
                Some(test_name)
            }
        })
        .collect();

    let mut next_steps = Vec::new();
    if pattern.is_some() {
        next_steps.push("Run pattern_rewrite with dry_run=true before applying edits".to_string());
    }
    if affected_symbols.is_empty() {
        next_steps.push("Resolve the target symbol or file before editing".to_string());
    } else {
        next_steps.push("Review the resolved callers and callees before editing".to_string());
    }
    if !related_tests.is_empty() {
        next_steps.push("Run related tests after applying changes".to_string());
    }

    let confidence = if affected_symbols.is_empty() || target_files.is_empty() {
        "low"
    } else {
        "high"
    };

    json!({
        "goal": goal,
        "target_files": target_files,
        "affected_symbols": affected_symbols,
        "related_tests": related_tests,
        "risks": risks,
        "confidence": confidence,
        "next_steps": next_steps,
    })
}
