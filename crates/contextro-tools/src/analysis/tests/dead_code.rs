use super::*;

#[test]
fn test_focus_returns_error_for_missing_path() {
    let graph = CodeGraph::new();
    let result = handle_focus(&json!({"path":"missing/file.rs"}), &graph, None);
    assert!(result.get("error").is_some());
}

#[test]
fn test_analyze_returns_error_for_missing_path() {
    let graph = CodeGraph::new();
    let result = handle_analyze(&json!({"path":"missing/dir"}), &graph, None);
    assert!(result.get("error").is_some());
}

#[test]
fn test_dead_code_skips_pytest_fixture_functions() {
    let dir = temp_dir("fixtures");
    let file = dir.join("conftest.py");
    std::fs::write(
        &file,
        "@pytest.fixture\nasync def browser():\n    return object()\n",
    )
    .unwrap();

    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "fixture".into(),
        name: "browser".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: file.to_string_lossy().to_string(),
            start_line: 2,
            end_line: 3,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        ..Default::default()
    });

    let result = handle_dead_code(&json!({}), &graph, Some(dir.to_string_lossy().as_ref()));
    assert_eq!(result["total"], 0);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_dead_code_skips_public_methods_unless_requested() {
    let dir = temp_dir("public-api");
    let file = dir.join("actor.py");
    std::fs::write(
        &file,
        "class BrowserSession:\n    def click(self):\n        pass\n",
    )
    .unwrap();

    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "click".into(),
        name: "click".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: file.to_string_lossy().to_string(),
            start_line: 2,
            end_line: 3,
            start_column: 0,
            end_column: 0,
            language: "python".into(),
        },
        language: "python".into(),
        parent: Some("BrowserSession".into()),
        ..Default::default()
    });

    let default_result = handle_dead_code(&json!({}), &graph, Some(dir.to_string_lossy().as_ref()));
    let include_public_api = handle_dead_code(
        &json!({"include_public_api": true}),
        &graph,
        Some(dir.to_string_lossy().as_ref()),
    );

    assert_eq!(default_result["total"], 0);
    assert_eq!(default_result["skipped_public_api"], 1);
    assert_eq!(include_public_api["total"], 1);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_dead_code_supports_path_and_exclude_filters() {
    let dir = temp_dir("filters");
    let src = dir.join("src");
    let vendor = dir.join("vendor");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&vendor).unwrap();
    let src_file = src.join("app.py");
    let vendor_file = vendor.join("shim.py");
    std::fs::write(&src_file, "def alpha():\n    pass\n").unwrap();
    std::fs::write(&vendor_file, "def beta():\n    pass\n").unwrap();

    let graph = CodeGraph::new();
    for (id, name, file_path) in [
        ("alpha", "alpha", src_file.to_string_lossy().to_string()),
        ("beta", "beta", vendor_file.to_string_lossy().to_string()),
    ] {
        graph.add_node(UniversalNode {
            id: id.into(),
            name: name.into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path,
                start_line: 1,
                end_line: 2,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });
    }

    let scoped = handle_dead_code(
        &json!({"path": src.to_string_lossy(), "limit": 10}),
        &graph,
        Some(dir.to_string_lossy().as_ref()),
    );
    let excluded = handle_dead_code(
        &json!({"exclude_paths": [vendor.to_string_lossy().to_string()]}),
        &graph,
        Some(dir.to_string_lossy().as_ref()),
    );

    assert_eq!(scoped["total"], 1);
    assert_eq!(scoped["dead_symbols"][0]["name"], "alpha");
    assert_eq!(excluded["total"], 1);
    assert_eq!(excluded["dead_symbols"][0]["name"], "alpha");
    assert_eq!(excluded["skipped_excluded"], 1);

    let _ = std::fs::remove_dir_all(dir);
}
