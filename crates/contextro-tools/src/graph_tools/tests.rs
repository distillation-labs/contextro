use super::*;
use contextro_core::graph::{
    RelationshipType, UniversalLocation, UniversalNode, UniversalRelationship,
};
use contextro_core::NodeType;

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
