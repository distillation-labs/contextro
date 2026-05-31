use super::*;
use contextro_core::graph::{
    RelationshipType, UniversalLocation, UniversalNode, UniversalRelationship,
};
use contextro_core::NodeType;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn sample_node(id: &str, name: &str, file: &str, line: u32) -> UniversalNode {
    UniversalNode {
        id: id.into(),
        name: name.into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: file.into(),
            start_line: line,
            end_line: line,
            start_column: 0,
            end_column: 0,
            language: "rust".into(),
        },
        language: "rust".into(),
        ..Default::default()
    }
}

#[test]
fn test_impact_results_are_monotonic_with_depth() {
    let graph = CodeGraph::new();
    let file = "/tmp/repo/src/session.py";

    for (id, name, line) in [
        ("leaf", "BrowserSession", 10_u32),
        ("mid", "create_browser_session", 20_u32),
        ("root", "start_app", 30_u32),
    ] {
        graph.add_node(UniversalNode {
            id: id.into(),
            name: name.into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: file.into(),
                start_line: line,
                end_line: line,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });
    }

    graph.add_relationship(UniversalRelationship {
        id: "rel-mid-leaf".into(),
        source_id: "mid".into(),
        target_id: "leaf".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });
    graph.add_relationship(UniversalRelationship {
        id: "rel-root-mid".into(),
        source_id: "root".into(),
        target_id: "mid".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });

    let depth_one = handle_impact(
        &json!({"symbol_name":"BrowserSession","max_depth":1}),
        &graph,
        Some("/tmp/repo"),
    );
    let depth_three = handle_impact(
        &json!({"symbol_name":"BrowserSession","max_depth":3}),
        &graph,
        Some("/tmp/repo"),
    );

    assert_eq!(depth_one["total"], 1);
    assert_eq!(depth_one["total_impacted"], 1);
    assert_eq!(depth_three["total"], 2);
    assert_eq!(depth_three["total_impacted"], 2);

    let shallow = depth_one["impacted"]
        .as_array()
        .expect("depth-one impacted list")
        .iter()
        .filter_map(|entry| entry["name"].as_str())
        .collect::<Vec<_>>();
    let deep = depth_three["impacted"]
        .as_array()
        .expect("depth-three impacted list")
        .iter()
        .filter_map(|entry| entry["name"].as_str())
        .collect::<Vec<_>>();

    assert!(deep.len() >= shallow.len());
    for name in shallow {
        assert!(deep.contains(&name));
    }
}

#[test]
fn test_impact_reports_default_depth_and_explicit_depth_hint() {
    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "leaf".into(),
        name: "BrowserSession".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: "/tmp/repo/src/session.py".into(),
            start_line: 10,
            end_line: 10,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });

    let default_result = handle_impact(
        &json!({"symbol_name":"BrowserSession"}),
        &graph,
        Some("/tmp/repo"),
    );
    let explicit_result = handle_impact(
        &json!({"symbol_name":"BrowserSession","max_depth":3}),
        &graph,
        Some("/tmp/repo"),
    );

    assert_eq!(default_result["default_depth"], 5);
    assert_eq!(default_result["total"], 0);
    assert!(default_result.get("depth_hint").is_none());
    assert_eq!(explicit_result["default_depth"], 5);
    assert_eq!(explicit_result["total"], 0);
    assert!(explicit_result["depth_hint"]
        .as_str()
        .unwrap_or("")
        .contains("narrower impact set"));
}

#[test]
fn test_find_callers_respects_limit() {
    let graph = CodeGraph::new();
    let file = "/tmp/repo/src/lib.rs";
    graph.add_node(sample_node("target", "dispatch", file, 10));
    graph.add_node(sample_node("caller-1", "call_one", file, 20));
    graph.add_node(sample_node("caller-2", "call_two", file, 30));
    graph.add_relationship(UniversalRelationship {
        id: "rel-1".into(),
        source_id: "caller-1".into(),
        target_id: "target".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });
    graph.add_relationship(UniversalRelationship {
        id: "rel-2".into(),
        source_id: "caller-2".into(),
        target_id: "target".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });

    let result = handle_find_callers(
        &json!({"symbol_name":"dispatch","limit":1}),
        &graph,
        Some("/tmp/repo"),
    );

    assert_eq!(result["limit"], 1);
    assert_eq!(result["total"], 1);
}

#[test]
fn test_find_callees_respects_limit() {
    let graph = CodeGraph::new();
    let file = "/tmp/repo/src/lib.rs";
    graph.add_node(sample_node("target", "dispatch", file, 10));
    graph.add_node(sample_node("callee-1", "handle_one", file, 20));
    graph.add_node(sample_node("callee-2", "handle_two", file, 30));
    graph.add_relationship(UniversalRelationship {
        id: "rel-1".into(),
        source_id: "target".into(),
        target_id: "callee-1".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });
    graph.add_relationship(UniversalRelationship {
        id: "rel-2".into(),
        source_id: "target".into(),
        target_id: "callee-2".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });

    let result = handle_find_callees(
        &json!({"symbol_name":"dispatch","limit":1}),
        &graph,
        Some("/tmp/repo"),
    );

    assert_eq!(result["limit"], 1);
    assert_eq!(result["total"], 1);
}

#[test]
fn test_test_for_prioritizes_graph_backed_and_inline_tests() {
    let repo = temp_repo_dir("test-for-direct");
    let source = repo.join("src/session.rs");
    let external_test = repo.join("tests/session.test.rs");
    write_file(
        &source,
        "pub fn BrowserSession() {}\n#[cfg(test)] mod tests { #[test] fn inline_browser_session() {} }\n",
    );
    write_file(&external_test, "#[test]\nfn test_browser_session() {}\n");

    let graph = CodeGraph::new();
    graph.add_node(sample_node(
        "target",
        "BrowserSession",
        source.to_string_lossy().as_ref(),
        10,
    ));
    graph.add_node(sample_node(
        "external-test",
        "test_browser_session",
        external_test.to_string_lossy().as_ref(),
        3,
    ));
    graph.add_relationship(UniversalRelationship {
        id: "rel-test-target".into(),
        source_id: "external-test".into(),
        target_id: "target".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });

    let result = handle_test_for(
        &json!({"symbol_name":"BrowserSession","limit":10}),
        &graph,
        Some(repo.to_string_lossy().as_ref()),
    );
    let tests = result["tests"].as_array().expect("test list");
    assert_eq!(result["candidate_total"], 2);
    assert_eq!(tests[0]["file"], "tests/session.test.rs");
    assert!(array_contains(&tests[0]["signals"], "direct_call"));
    assert!(array_contains(&tests[0]["signals"], "exact_stem"));
    assert!(tests.iter().any(|entry| entry["file"] == "src/session.rs"
        && array_contains(&entry["signals"], "inline_test")));

    let _ = fs::remove_dir_all(repo);
}

#[test]
fn test_test_for_uses_token_overlap_without_call_edges() {
    let repo = temp_repo_dir("test-for-heuristic");
    let source = repo.join("src/browser/session.py");
    let heuristic_test = repo.join("tests/browser/test_browser_session_flow.py");
    write_file(&source, "def BrowserSession():\n    return None\n");
    write_file(
        &heuristic_test,
        "def test_browser_session_flow():\n    assert True\n",
    );

    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "target".into(),
        name: "BrowserSession".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: source.to_string_lossy().to_string(),
            start_line: 1,
            end_line: 1,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });
    graph.add_node(UniversalNode {
        id: "heuristic-test".into(),
        name: "test_browser_session_flow".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: heuristic_test.to_string_lossy().to_string(),
            start_line: 1,
            end_line: 1,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });

    let result = handle_test_for(
        &json!({"symbol_name":"BrowserSession","limit":10}),
        &graph,
        Some(repo.to_string_lossy().as_ref()),
    );
    let tests = result["tests"].as_array().expect("test list");
    assert!(tests.iter().any(|entry| entry["file"]
        == "tests/browser/test_browser_session_flow.py"
        && array_contains(&entry["signals"], "token_overlap")));

    let _ = fs::remove_dir_all(repo);
}

#[test]
fn test_test_for_invalidates_cache_when_graph_changes() {
    let repo = temp_repo_dir("test-for-cache-refresh");
    let source = repo.join("src/session.rs");
    let external_test = repo.join("tests/session.test.rs");
    write_file(&source, "pub fn BrowserSession() {}\n");

    let graph = CodeGraph::new();
    graph.add_node(sample_node(
        "target",
        "BrowserSession",
        source.to_string_lossy().as_ref(),
        10,
    ));

    let first = handle_test_for(
        &json!({"symbol_name":"BrowserSession","limit":10}),
        &graph,
        Some(repo.to_string_lossy().as_ref()),
    );
    assert_eq!(first["candidate_total"], 0);

    write_file(&external_test, "#[test]\nfn test_browser_session() {}\n");
    graph.add_node(sample_node(
        "external-test",
        "test_browser_session",
        external_test.to_string_lossy().as_ref(),
        3,
    ));

    let second = handle_test_for(
        &json!({"symbol_name":"BrowserSession","limit":10}),
        &graph,
        Some(repo.to_string_lossy().as_ref()),
    );
    let tests = second["tests"].as_array().expect("test list");
    assert_eq!(second["candidate_total"], 1);
    assert_eq!(tests[0]["file"], "tests/session.test.rs");

    let _ = fs::remove_dir_all(repo);
}

fn array_contains(value: &serde_json::Value, needle: &str) -> bool {
    value
        .as_array()
        .map(|values| values.iter().any(|entry| entry.as_str() == Some(needle)))
        .unwrap_or(false)
}

fn temp_repo_dir(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("contextro-{prefix}-{unique}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(path, content).expect("write file");
}
