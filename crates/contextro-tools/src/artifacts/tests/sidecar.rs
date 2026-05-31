use super::*;

#[test]
fn test_sidecar_export_matches_relative_indexed_paths() {
    let root = temp_dir("sidecars");
    let codebase = root.join("repo");
    std::fs::create_dir_all(codebase.join("src")).unwrap();
    let output_dir = root.join("out");

    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "browser-session".into(),
        name: "BrowserSession".into(),
        node_type: NodeType::Class,
        location: UniversalLocation {
            file_path: "src/session.py".into(),
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
    let result = handle_sidecar_export(
        &json!({"path":"src","output_dir": output_dir.to_string_lossy()}),
        &graph,
        Some(base.as_str()),
        1,
    );

    assert_eq!(result["sidecars"], 1);
    assert!(output_dir.join("src/session.py.graph.md").exists());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn test_sidecar_export_errors_when_path_matches_no_indexed_files() {
    let root = temp_dir("sidecars-missing");
    let codebase = root.join("repo");
    std::fs::create_dir_all(codebase.join("src")).unwrap();
    std::fs::create_dir_all(codebase.join("out")).unwrap();

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
    let result = handle_sidecar_export(
        &json!({"path":"out","output_dir": codebase.join("export").to_string_lossy()}),
        &graph,
        Some(base.as_str()),
        1,
    );

    assert!(result["error"]
        .as_str()
        .unwrap_or("")
        .contains("No indexed files matched path"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn test_sidecar_export_skips_rewriting_current_outputs() {
    let root = temp_dir("sidecars-cache");
    let codebase = root.join("repo");
    std::fs::create_dir_all(codebase.join("src")).unwrap();
    let output_dir = root.join("out");

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
    let args = json!({"path":"src","output_dir": output_dir.to_string_lossy()});
    handle_sidecar_export(&args, &graph, Some(base.as_str()), 9);

    let sidecar_path = output_dir.join("src/session.py.graph.md");
    let first_modified = std::fs::metadata(&sidecar_path)
        .unwrap()
        .modified()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(25));

    handle_sidecar_export(&args, &graph, Some(base.as_str()), 9);

    let second_modified = std::fs::metadata(&sidecar_path)
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(first_modified, second_modified);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn test_skill_prompt_mentions_updated_parameter_contracts() {
    let result = handle_introspect(&json!({"tool":"commit_history"}));
    let parameters = result["parameters"].as_array().expect("parameters array");

    assert!(parameters
        .iter()
        .any(|parameter| parameter.as_str() == Some("author: optional author substring filter")));

    let recall = handle_introspect(&json!({"tool":"recall"}));
    assert!(recall["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .any(|parameter| parameter
            .as_str()
            .unwrap_or("")
            .contains("empty string lists recent memories")));
}
