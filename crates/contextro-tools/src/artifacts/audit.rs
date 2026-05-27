use std::collections::HashMap;

use crate::analysis::{is_generic_symbol_name, is_test_file, strip_base};
use contextro_engines::graph::CodeGraph;
use serde_json::{json, Value};

pub(crate) const AUDIT_CONNECTION_THRESHOLD: usize = 10;
pub(crate) const AUDIT_FILE_SYMBOL_THRESHOLD: usize = 30;
pub(crate) const AUDIT_EVIDENCE_LIMIT: usize = 3;

fn audit_quality_score(
    recommendation_count: usize,
    max_connection_overage: usize,
    max_file_overage: usize,
) -> usize {
    if recommendation_count == 0 {
        95
    } else {
        85usize
            .saturating_sub(recommendation_count * 5)
            .saturating_sub(max_connection_overage / AUDIT_CONNECTION_THRESHOLD)
            .saturating_sub(max_file_overage / AUDIT_FILE_SYMBOL_THRESHOLD)
    }
}

fn is_audit_noise_file(file_path: &str) -> bool {
    is_test_file(file_path)
        || file_path.starts_with("tests/")
        || file_path.starts_with("test/")
        || file_path.starts_with("__tests__/")
        || file_path.starts_with("e2e/")
        || file_path.starts_with("spec/")
}

fn audit_command_arg(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

/// Generate an audit report with recommendations.
pub fn handle_audit(graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let snapshot = graph.snapshot();
    let degree_by_id: HashMap<String, (usize, usize)> = snapshot
        .nodes()
        .iter()
        .map(|node| (node.id.clone(), snapshot.degree(&node.id)))
        .collect();
    let all_nodes = snapshot.into_nodes();
    let total_symbols = all_nodes.len();
    let nodes: Vec<_> = all_nodes
        .into_iter()
        .filter(|node| !is_generic_symbol_name(&node.name))
        .filter(|node| !is_audit_noise_file(&node.location.file_path))
        .collect();
    let mut recommendations: Vec<Value> = Vec::new();

    // Check for high-complexity symbols and keep only the top offenders.
    let mut high_conn: Vec<_> = Vec::new();
    for node in &nodes {
        let (in_d, out_d) = degree_by_id.get(&node.id).copied().unwrap_or((0, 0));
        let connections = in_d + out_d;
        if connections > AUDIT_CONNECTION_THRESHOLD {
            high_conn.push((
                node.name.clone(),
                strip_base(&node.location.file_path, codebase),
                connections,
            ));
        }
    }
    high_conn.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    if !high_conn.is_empty() {
        let affected_count = high_conn.len();
        recommendations.push(json!({
            "severity": "medium",
            "category": "complexity",
            "message": format!(
                "{} symbols have >{} connections; inspect the top offenders below",
                affected_count,
                AUDIT_CONNECTION_THRESHOLD
            ),
            "threshold": AUDIT_CONNECTION_THRESHOLD,
            "affected_count": affected_count,
            "evidence": high_conn
                .iter()
                .take(AUDIT_EVIDENCE_LIMIT)
                .map(|(symbol, file, connections)| {
                    json!({
                        "symbol": symbol,
                        "file": file,
                        "connections": connections,
                        "follow_up": [
                            format!("explain({{\"symbol_name\":{}}})", audit_command_arg(symbol)),
                            format!(
                                "impact({{\"symbol_name\":{},\"max_depth\":3}})",
                                audit_command_arg(symbol)
                            ),
                        ]
                    })
                })
                .collect::<Vec<_>>(),
        }));
    }

    // Check file concentration and surface the biggest files first.
    let mut file_counts: HashMap<String, usize> = HashMap::new();
    for node in nodes {
        *file_counts
            .entry(node.location.file_path.clone())
            .or_default() += 1;
    }
    let mut large_files: Vec<_> = file_counts
        .into_iter()
        .filter(|(_, count)| *count > AUDIT_FILE_SYMBOL_THRESHOLD)
        .collect();
    large_files.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    if !large_files.is_empty() {
        let affected_count = large_files.len();
        recommendations.push(json!({
            "severity": "low",
            "category": "structure",
            "message": format!(
                "{} files have >{} symbols; inspect the largest files below",
                affected_count,
                AUDIT_FILE_SYMBOL_THRESHOLD
            ),
            "threshold": AUDIT_FILE_SYMBOL_THRESHOLD,
            "affected_count": affected_count,
            "evidence": large_files
                .iter()
                .take(AUDIT_EVIDENCE_LIMIT)
                .map(|(file, symbols)| {
                    let file = strip_base(file, codebase);
                    json!({
                        "file": file,
                        "symbols": symbols,
                        "follow_up": [
                            format!("analyze({{\"path\":{}}})", audit_command_arg(&file)),
                            format!("focus({{\"path\":{}}})", audit_command_arg(&file)),
                        ]
                    })
                })
                .collect::<Vec<_>>(),
        }));
    }

    let max_connection_overage = high_conn
        .first()
        .map(|(_, _, connections)| connections.saturating_sub(AUDIT_CONNECTION_THRESHOLD))
        .unwrap_or(0);
    let max_file_overage = large_files
        .first()
        .map(|(_, symbols)| symbols.saturating_sub(AUDIT_FILE_SYMBOL_THRESHOLD))
        .unwrap_or(0);
    let quality_score = audit_quality_score(
        recommendations.len(),
        max_connection_overage,
        max_file_overage,
    );

    json!({
        "status": "complete",
        "quality_score": quality_score,
        "total_symbols": total_symbols,
        "recommendations": recommendations,
    })
}
