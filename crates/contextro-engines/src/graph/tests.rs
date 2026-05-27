use super::*;
use contextro_core::graph::{NodeType, UniversalLocation};

fn make_node(id: &str, name: &str) -> UniversalNode {
    UniversalNode {
        id: id.into(),
        name: name.into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: "test.rs".into(),
            start_line: 1,
            end_line: 10,
            start_column: 0,
            end_column: 0,
            language: "rust".into(),
        },
        language: "rust".into(),
        line_count: 10,
        ..Default::default()
    }
}

#[test]
fn test_token_index_fuzzy_search() {
    let graph = CodeGraph::new();
    graph.add_node(make_node("1", "createUser"));
    graph.add_node(make_node("2", "deleteUser"));
    graph.add_node(make_node("3", "authenticate"));

    // Fuzzy search by token
    let results = graph.find_nodes_by_name("user", false);
    assert_eq!(results.len(), 2);

    let results = graph.find_nodes_by_name("create", false);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "createUser");
}

#[test]
fn test_tokenize_name() {
    assert_eq!(
        tokenize_name("createUser"),
        vec!["create", "user", "createuser"]
    );
    assert_eq!(
        tokenize_name("find_nodes_by_name"),
        vec!["find", "nodes", "by", "name", "find_nodes_by_name"]
    );
}

#[test]
fn test_snapshot_preserves_nodes_and_degrees() {
    let graph = CodeGraph::new();
    graph.add_node(make_node("1", "createUser"));
    graph.add_node(make_node("2", "deleteUser"));
    graph.add_relationship(UniversalRelationship {
        id: "r1".into(),
        source_id: "1".into(),
        target_id: "2".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });

    let snapshot = graph.snapshot();

    assert_eq!(snapshot.nodes().len(), 2);
    assert_eq!(snapshot.degree("1"), (0, 1));
    assert_eq!(snapshot.degree("2"), (1, 0));
}

#[test]
fn test_restore_snapshot_rehydrates_relationship_indexes() {
    let graph = CodeGraph::new();
    graph.add_node(make_node("1", "createUser"));
    graph.add_node(make_node("2", "deleteUser"));
    graph.add_relationship(UniversalRelationship {
        id: "r1".into(),
        source_id: "1".into(),
        target_id: "2".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });
    graph.compute_pagerank();

    let snapshot = graph.snapshot();
    let restored = CodeGraph::new();
    restored.restore_snapshot(&snapshot);

    assert_eq!(restored.node_count(), 2);
    assert_eq!(restored.relationship_count(), 1);
    assert_eq!(restored.get_node_degree("1"), (0, 1));
    assert_eq!(restored.get_node_degree("2"), (1, 0));
    assert_eq!(restored.find_nodes_by_name("create", false).len(), 1);
    assert!(restored.get_pagerank("1") > 0.0);
}

#[test]
fn test_get_nodes_by_file_returns_file_scoped_nodes() {
    let graph = CodeGraph::new();
    let mut first = make_node("1", "createUser");
    first.location.file_path = "src/a.rs".into();
    let mut second = make_node("2", "deleteUser");
    second.location.file_path = "src/a.rs".into();
    let mut third = make_node("3", "authenticate");
    third.location.file_path = "src/b.rs".into();

    graph.add_node(first);
    graph.add_node(second);
    graph.add_node(third);

    let nodes = graph.get_nodes_by_file("src/a.rs");
    let names: Vec<&str> = nodes.iter().map(|node| node.name.as_str()).collect();

    assert_eq!(names, vec!["createUser", "deleteUser"]);
}
