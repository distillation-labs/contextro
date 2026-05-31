use super::*;

#[test]
fn test_introspect_tool_filter_returns_exact_tool_docs() {
    let result = handle_introspect(&json!({"tool":"search"}));
    let parameters = result["parameters"].as_array().expect("parameters array");

    assert_eq!(result["name"], "search");
    assert_eq!(result["tool"], "search");
    assert_eq!(
        result["description"],
        "Hybrid, vector, or BM25 code search."
    );
    assert!(parameters.iter().any(|parameter| parameter.as_str()
        == Some("query (required): search text or symbol-like identifier")));
}

#[test]
fn test_skill_prompt_includes_parameter_docs_and_archive_ref_note() {
    let result = handle_skill_prompt();
    let conventions = result["parameter_conventions"]
        .as_array()
        .expect("parameter conventions array");
    let core_tools = result["core_tools"].as_array().expect("core tools array");

    assert!(conventions
        .iter()
        .any(|note| note.as_str().unwrap_or("").contains("arc_")));
    assert!(core_tools
        .iter()
        .any(|tool| tool["tool"] == "search" && tool["parameters"].is_array()));
}

#[test]
fn test_introspect_list_includes_name_alias() {
    let result = handle_introspect(&json!({}));
    let tools = result["tools"].as_array().expect("tools array");

    assert!(tools
        .iter()
        .any(|tool| tool["name"] == "search" && tool["tool"] == "search"));
}

#[test]
fn test_docs_bundle_writes_rich_overview_markdown() {
    let root = temp_dir("docs-bundle");
    let codebase = root.join("repo");
    std::fs::create_dir_all(codebase.join("src/browser")).unwrap();

    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "browser-session".into(),
        name: "BrowserSession".into(),
        node_type: NodeType::Class,
        location: UniversalLocation {
            file_path: codebase
                .join("src/browser/session.py")
                .to_string_lossy()
                .to_string(),
            start_line: 1,
            end_line: 20,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });
    graph.add_node(UniversalNode {
        id: "get-session".into(),
        name: "get_or_create_session".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: codebase
                .join("src/browser/session.py")
                .to_string_lossy()
                .to_string(),
            start_line: 22,
            end_line: 35,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });
    graph.add_relationship(UniversalRelationship {
        id: "rel-1".into(),
        source_id: "get-session".into(),
        target_id: "browser-session".into(),
        relationship_type: RelationshipType::Calls,
        strength: 1.0,
    });

    let base = codebase.to_string_lossy().to_string();
    let result = handle_docs_bundle(
        &json!({"output_dir":"docs"}),
        &graph,
        Some(base.as_str()),
        1,
    );
    assert_eq!(result["status"], "generated");

    let overview = std::fs::read_to_string(codebase.join("docs/overview.md")).unwrap();
    assert!(overview.contains("## Summary"));
    assert!(overview.contains("## Languages"));
    assert!(overview.contains("## Top Files"));
    assert!(overview.contains("## Hub Symbols"));
    assert!(overview.contains("BrowserSession"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn test_docs_bundle_requires_indexed_graph() {
    let root = temp_dir("docs-bundle-empty");
    let codebase = root.join("repo");
    std::fs::create_dir_all(&codebase).unwrap();
    let graph = CodeGraph::new();

    let base = codebase.to_string_lossy().to_string();
    let result = handle_docs_bundle(
        &json!({"output_dir":"docs"}),
        &graph,
        Some(base.as_str()),
        1,
    );

    assert_eq!(
        result["error"],
        "No indexed graph loaded. Run index(path) before docs_bundle."
    );
    assert!(
        !codebase.join("docs/overview.md").exists(),
        "docs bundle should not write placeholder docs when no graph is loaded"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn test_docs_bundle_skips_rewriting_current_outputs() {
    let root = temp_dir("docs-bundle-cache");
    let codebase = root.join("repo");
    std::fs::create_dir_all(codebase.join("src")).unwrap();

    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "browser-session".into(),
        name: "BrowserSession".into(),
        node_type: NodeType::Class,
        location: UniversalLocation {
            file_path: codebase
                .join("src/session.py")
                .to_string_lossy()
                .to_string(),
            start_line: 1,
            end_line: 20,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });

    let base = codebase.to_string_lossy().to_string();
    let args = json!({"output_dir":"docs"});
    handle_docs_bundle(&args, &graph, Some(base.as_str()), 7);

    let overview_path = codebase.join("docs/overview.md");
    let first_modified = std::fs::metadata(&overview_path)
        .unwrap()
        .modified()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(25));

    handle_docs_bundle(&args, &graph, Some(base.as_str()), 7);

    let second_modified = std::fs::metadata(&overview_path)
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(first_modified, second_modified);

    let _ = std::fs::remove_dir_all(root);
}
