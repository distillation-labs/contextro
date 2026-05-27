use super::*;

#[test]
fn test_architecture_filters_generic_python_builtins() {
    let graph = CodeGraph::new();
    let file = "/tmp/browser_use/session.py";

    graph.add_node(UniversalNode {
        id: "append".into(),
        name: "append".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: file.into(),
            start_line: 10,
            end_line: 12,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });
    graph.add_node(UniversalNode {
        id: "__init__".into(),
        name: "__init__".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: file.into(),
            start_line: 20,
            end_line: 25,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });
    graph.add_node(UniversalNode {
        id: "browser-session".into(),
        name: "BrowserSession".into(),
        node_type: NodeType::Class,
        location: UniversalLocation {
            file_path: file.into(),
            start_line: 30,
            end_line: 60,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });
    graph.add_node(UniversalNode {
        id: "caller-a".into(),
        name: "make_session".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: file.into(),
            start_line: 70,
            end_line: 72,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });
    graph.add_node(UniversalNode {
        id: "caller-b".into(),
        name: "close_session".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: file.into(),
            start_line: 80,
            end_line: 82,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });

    graph.add_relationship(UniversalRelationship {
        id: "rel-1".into(),
        source_id: "caller-a".into(),
        target_id: "append".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });
    graph.add_relationship(UniversalRelationship {
        id: "rel-2".into(),
        source_id: "caller-b".into(),
        target_id: "__init__".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });
    graph.add_relationship(UniversalRelationship {
        id: "rel-3".into(),
        source_id: "caller-a".into(),
        target_id: "browser-session".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });
    graph.add_relationship(UniversalRelationship {
        id: "rel-4".into(),
        source_id: "caller-b".into(),
        target_id: "browser-session".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });

    let result = handle_architecture(&json!({}), &graph, None);
    let names = result["hub_symbols"]
        .as_array()
        .expect("hub symbols")
        .iter()
        .filter_map(|entry| entry["name"].as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"BrowserSession"));
    assert!(!names.contains(&"append"));
    assert!(!names.contains(&"__init__"));
}

#[test]
fn test_architecture_respects_limit() {
    let graph = CodeGraph::new();
    for idx in 0..3 {
        graph.add_node(UniversalNode {
            id: format!("node-{idx}"),
            name: format!("Symbol{idx}"),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "/tmp/repo/src/lib.rs".into(),
                start_line: idx + 1,
                end_line: idx + 1,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
    }

    let result = handle_architecture(&json!({"limit":2}), &graph, None);
    assert_eq!(result["limit"], 2);
    assert_eq!(result["hub_symbols"].as_array().unwrap().len(), 2);
}

#[test]
fn test_analyze_respects_min_connections_and_top_n() {
    let graph = CodeGraph::new();
    let file = "/tmp/repo/src/lib.rs";
    graph.add_node(UniversalNode {
        id: "dispatch".into(),
        name: "dispatch".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: file.into(),
            start_line: 1,
            end_line: 1,
            start_column: 0,
            end_column: 0,
            language: "rust".into(),
        },
        language: "rust".into(),
        ..Default::default()
    });
    for idx in 0..3 {
        graph.add_node(UniversalNode {
            id: format!("caller-{idx}"),
            name: format!("caller_{idx}"),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: file.into(),
                start_line: (idx + 2) as u32,
                end_line: (idx + 2) as u32,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
        graph.add_relationship(UniversalRelationship {
            id: format!("rel-{idx}"),
            source_id: format!("caller-{idx}"),
            target_id: "dispatch".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });
    }

    let result = handle_analyze(&json!({"min_connections":3,"top_n":1}), &graph, None);
    assert_eq!(result["min_connections"], 3);
    assert_eq!(result["top_n"], 1);
    assert_eq!(
        result["high_connectivity_symbols"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(result["high_connectivity_symbols"][0]["name"], "dispatch");
}
